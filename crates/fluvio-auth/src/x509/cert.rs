use anyhow::{Context, Result};

use bon::Builder;
use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use openssl::stack::Stack;
use openssl::x509::extension::{
    AuthorityKeyIdentifier, BasicConstraints, KeyUsage, SubjectAlternativeName,
    SubjectKeyIdentifier,
};
use openssl::x509::X509Builder;
use openssl::x509::X509NameBuilder;
use openssl::x509::X509Req;
use openssl::x509::X509ReqBuilder;
use openssl::x509::X509;

type PrivateKey = PKey<Private>;

/// Configuration for generating a ClusterCA
#[derive(Builder)]
pub struct ClusterCertConfig {
    #[builder(into)]
    cn: String, // common name
    #[builder(into)]
    domain: String, // domain
    #[builder(default = 4096)]
    key_len: u32,
    #[builder(default = 365)]
    expired_days: u32,
}

impl ClusterCertConfig {
    /// Generate an X509 CA certificate
    ///
    /// The CA certificate is used to sign a ServerCert and a
    /// ClientCert pair, which are used to grant a user access
    /// to their Fluvio cluster.
    pub fn generate(self) -> Result<ClusterCA> {
        let private_key = generate_private_key(self.key_len)?;

        let mut x509_name = X509NameBuilder::new()?;
        x509_name.append_entry_by_text("CN", &self.cn)?;

        let x509_name = x509_name.build();

        let mut builder = X509Builder::new()?;
        builder.set_version(2)?;
        let serial_number = {
            let mut serial = BigNum::new()?;
            serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
            serial.to_asn1_integer()?
        };
        builder.set_serial_number(&serial_number)?;
        builder.set_subject_name(&x509_name)?;
        builder.set_issuer_name(&x509_name)?;
        builder.set_pubkey(&private_key)?;
        builder.set_not_before(time_now()?.as_ref())?;
        builder.set_not_after(expired_days(self.expired_days)?.as_ref())?;

        let subject_key_identifier =
            SubjectKeyIdentifier::new().build(&builder.x509v3_context(None, None))?;
        builder.append_extension(subject_key_identifier)?;

        let authority_key_identifier = AuthorityKeyIdentifier::new()
            .issuer(false)
            .keyid(false)
            .build(&builder.x509v3_context(None, None))?;
        builder.append_extension(authority_key_identifier)?;

        let basic = BasicConstraints::new().ca().critical().build()?;
        builder.append_extension(basic)?;

        let subject_alternate_names = SubjectAlternativeName::new()
            .dns(&self.domain)
            .build(&builder.x509v3_context(None, None))?;
        builder.append_extension(subject_alternate_names)?;

        builder.sign(&private_key, MessageDigest::sha256())?;
        let ca_cert = builder.build();

        Ok(ClusterCA {
            cert: ca_cert,
            key: private_key,
        })
    }
}

/// A CA cert and private key
#[derive(Debug, Clone)]
pub struct ClusterCA {
    cert: X509,
    key: PrivateKey,
}

impl ClusterCA {
    pub fn builder() -> ClusterCertConfigBuilder {
        ClusterCertConfig::builder()
    }

    pub fn new(cert: X509, key: PrivateKey) -> Self {
        Self { cert, key }
    }

    /// Gives the inner certificate as a PEM encoded String
    pub fn cert_pem(&self) -> Result<String> {
        let pem = self.cert.to_pem()?;
        let pem_string = String::from_utf8(pem)?;
        Ok(pem_string)
    }

    /// Gives the inner private key as a PEM encoded String
    pub fn key_pem(&self) -> Result<String> {
        let pem = self.key.rsa()?.private_key_to_pem()?;
        let pem_string = String::from_utf8(pem)?;
        Ok(pem_string)
    }

    /// Gives the inner certificate as DER bytes
    pub fn cert_der(&self) -> Result<Vec<u8>> {
        Ok(self.cert.to_der()?)
    }

    /// Gives the inner certificate as DER bytes
    pub fn key_der(&self) -> Result<Vec<u8>> {
        Ok(self.key.private_key_to_der()?)
    }

    pub fn domain(&self) -> Result<String> {
        Ok(self
            .cert
            .subject_alt_names()
            .context("missing subject alt names")?
            .into_iter()
            .next()
            .context("missing domain")?
            .dnsname()
            .context("missing dns name")?
            .to_owned())
    }

    fn common_name(&self) -> Result<String> {
        self.subject_name_field(Nid::COMMONNAME)
            .context("missing CN")
    }

    fn subject_name_field(&self, nid: Nid) -> Option<String> {
        self.cert
            .subject_name()
            .entries_by_nid(nid)
            .filter_map(|entry| {
                entry
                    .data()
                    .as_utf8()
                    .ok()
                    .map(|s| AsRef::<str>::as_ref(&s).to_owned())
            })
            .next()
    }
}

#[derive(Builder)]
pub struct ServerCertConfig {
    #[builder(into)]
    hostname: String,
    #[builder(into)]
    domain: String,
    #[builder(default = 4096)]
    key_len: u32,
    #[builder(default = 365)]
    expired_days: u32,
}

impl ServerCertConfig {
    /// Generate a server CSR
    fn generate(self, ca: &ClusterCA) -> Result<ServerCert> {
        let server_key = generate_private_key(self.key_len)?;

        let mut req_builder = X509ReqBuilder::new()?;
        req_builder.set_version(2)?;
        req_builder.set_pubkey(&server_key)?;

        let mut x509_name = X509NameBuilder::new()?;
        x509_name.append_entry_by_text("CN", &self.hostname)?;

        let x509_name = x509_name.build();
        req_builder.set_subject_name(&x509_name)?;

        let domain = &self.domain;
        let subject_alternate_names = SubjectAlternativeName::new()
            .dns(domain)
            // wildcard for spu
            .dns(&format!("*.{domain}"))
            .build(&req_builder.x509v3_context(None))?;

        let mut extension_stack = Stack::new().unwrap();
        extension_stack.push(subject_alternate_names)?;
        req_builder.add_extensions(&extension_stack)?;

        req_builder.sign(&server_key, MessageDigest::sha256())?;
        let req = req_builder.build();
        ServerCert::generate(ca, &req, server_key, self.expired_days)
    }
}

pub struct ServerCrtConfig {
    expired_days: u32,
}

/// A Server's certificate and private key
///
/// This is signed by some `ClusterCA`
pub struct ServerCert {
    cert: X509,
    key: PrivateKey,
}

impl ServerCert {
    pub fn new(cert: X509, key: PrivateKey) -> Self {
        Self { cert, key }
    }

    /// Generate a server CRT based on CA cert and private key
    fn generate(
        ca: &ClusterCA,
        server_csr: &X509Req,
        server_key: PrivateKey,
        days: u32,
    ) -> Result<Self> {
        let mut cert_builder = X509::builder()?;
        cert_builder.set_version(2)?;
        let serial_number = {
            let mut serial = BigNum::new()?;
            serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
            serial.to_asn1_integer()?
        };
        cert_builder.set_serial_number(&serial_number)?;
        cert_builder.set_subject_name(server_csr.subject_name())?;
        cert_builder.set_issuer_name(ca.cert.issuer_name())?;
        cert_builder.set_pubkey(&server_key)?;
        cert_builder.set_not_before(time_now()?.as_ref())?;
        cert_builder.set_not_after(expired_days(days)?.as_ref())?;

        let basic = BasicConstraints::new().critical().build()?;
        cert_builder.append_extension(basic)?;

        let key_usage = KeyUsage::new()
            .non_repudiation()
            .digital_signature()
            .build()?;
        cert_builder.append_extension(key_usage)?;

        let subject_key_identifier = SubjectKeyIdentifier::new()
            .build(&cert_builder.x509v3_context(Some(&ca.cert), None))?;
        cert_builder.append_extension(subject_key_identifier)?;

        let auth_key_identifier = AuthorityKeyIdentifier::new()
            .keyid(true)
            .issuer(true)
            .build(&cert_builder.x509v3_context(Some(&ca.cert), None))?;
        cert_builder.append_extension(auth_key_identifier)?;

        for ext in server_csr.extensions()? {
            cert_builder.append_extension(ext)?;
        }

        cert_builder.sign(&ca.key, MessageDigest::sha256())?;
        let cert = cert_builder.build();

        Ok(ServerCert {
            cert,
            key: server_key,
        })
    }

    /// Gives the inner certificate as a PEM encoded String
    pub fn cert_pem(&self) -> Result<String> {
        let pem = self.cert.to_pem()?;
        let pem_string = String::from_utf8(pem)?;
        Ok(pem_string)
    }

    /// Gives the inner private key as a PEM encoded String
    pub fn key_pem(&self) -> Result<String> {
        let pem = self.key.rsa()?.private_key_to_pem()?;
        let pem_string = String::from_utf8(pem)?;
        Ok(pem_string)
    }
}

#[derive(Builder)]
pub struct ClientCertConfig {
    #[builder(into)]
    principal: String,
    #[builder(default = 4096)]
    key_len: u32,
    #[builder(default = 365)]
    expired_days: u32,
}

impl ClientCertConfig {
    /// Generate a client CSR
    fn generate(self, cluster: &ClusterCA) -> Result<ClientCert> {
        let client_key = generate_private_key(self.key_len)?;

        let mut req_builder = X509ReqBuilder::new()?;
        req_builder.set_version(2)?;
        req_builder.set_pubkey(&client_key)?;

        let mut x509_name = X509NameBuilder::new()?;
        x509_name.append_entry_by_text("CN", &self.principal)?;

        let x509_name = x509_name.build();
        req_builder.set_subject_name(&x509_name)?;

        req_builder.sign(&client_key, MessageDigest::sha256())?;
        let req = req_builder.build();
        ClientCert::generate(cluster, &req, client_key, self.expired_days)
    }
}

/// A Client's certificate and private key
///
/// This is signed by some `ClusterCA`
pub struct ClientCert {
    cert: X509,
    key: PrivateKey,
}

impl ClientCert {
    pub fn new(cert: X509, key: PrivateKey) -> Self {
        Self { cert, key }
    }

    /// Generate a server CRT based on CA cert and private key
    fn generate(
        ca: &ClusterCA,
        client_csr: &X509Req,
        client_key: PrivateKey,
        days: u32,
    ) -> Result<Self> {
        let mut cert_builder = X509::builder()?;
        cert_builder.set_version(2)?;
        let serial_number = {
            let mut serial = BigNum::new()?;
            serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
            serial.to_asn1_integer()?
        };
        cert_builder.set_serial_number(&serial_number)?;
        cert_builder.set_subject_name(client_csr.subject_name())?;
        cert_builder.set_issuer_name(ca.cert.issuer_name())?;
        cert_builder.set_pubkey(&client_key)?;
        cert_builder.set_not_before(time_now()?.as_ref())?;
        cert_builder.set_not_after(expired_days(days)?.as_ref())?;

        let basic = BasicConstraints::new().critical().build()?;
        cert_builder.append_extension(basic)?;

        let key_usage = KeyUsage::new()
            .non_repudiation()
            .digital_signature()
            .build()?;
        cert_builder.append_extension(key_usage)?;

        let subject_key_identifier = SubjectKeyIdentifier::new()
            .build(&cert_builder.x509v3_context(Some(&ca.cert), None))?;
        cert_builder.append_extension(subject_key_identifier)?;

        let auth_key_identifier = AuthorityKeyIdentifier::new()
            .keyid(true)
            .issuer(true)
            .build(&cert_builder.x509v3_context(Some(&ca.cert), None))?;
        cert_builder.append_extension(auth_key_identifier)?;

        cert_builder.sign(&ca.key, MessageDigest::sha256())?;
        let cert = cert_builder.build();

        Ok(ClientCert {
            cert,
            key: client_key,
        })
    }

    /// Gives the inner certificate as a PEM encoded String
    pub fn cert_pem(&self) -> Result<String> {
        let pem = self.cert.to_pem()?;
        let pem_string = String::from_utf8(pem)?;
        Ok(pem_string)
    }

    /// Gives the inner private key as a PEM encoded String
    pub fn key_pem(&self) -> Result<String> {
        let pem = self.key.rsa()?.private_key_to_pem()?;
        let pem_string = String::from_utf8(pem)?;
        Ok(pem_string)
    }
}

#[derive(Debug, Clone)]
pub struct DistinguishedName {
    pub org: Option<String>,
    pub email: String,
    pub cn: String,
}

/// Generate a default private key with 4096 bits
fn generate_private_key(key_len: u32) -> Result<PrivateKey, ErrorStack> {
    let rsa = Rsa::generate(key_len)?;
    PKey::from_rsa(rsa)
}

/// Returns a value describing the current time
fn time_now() -> Result<Asn1Time, ErrorStack> {
    Asn1Time::days_from_now(0)
}

/// Returns a value describing the time fifteen years from now
fn expired_days(days: u32) -> Result<Asn1Time, ErrorStack> {
    Asn1Time::days_from_now(days)
}

#[cfg(test)]
mod tests {
    use crate::x509::CertMetadata;

    use super::*;
    use openssl::rsa::Padding;
    use openssl::x509::X509VerifyResult;

    const TEST_DOMAIN: &str = "fluvio.local";
    const TEST_HOSTNAME: &str = "test.fluvio.local";

    const TEST_PRINCIPAL: &str = "user1";

    #[test]
    fn test_cert_generation() {
        let ca_cert = ClusterCA::builder()
            .cn(TEST_HOSTNAME)
            .domain(TEST_DOMAIN)
            .build()
            .generate()
            .expect("should generate ClusterCA");

        let server_crt = ServerCertConfig::builder()
            .hostname(TEST_HOSTNAME)
            .domain(TEST_DOMAIN)
            .build()
            .generate(&ca_cert)
            .expect("should generate ServerCert");

        // Verify that the CA issued the server cert
        match ca_cert.cert.issued(&server_crt.cert) {
            X509VerifyResult::OK => (),
            _ => panic!("CA did not issue this ServerCert"),
        }

        let client_crt = ClientCertConfig::builder()
            .principal(TEST_PRINCIPAL)
            .build()
            .generate(&ca_cert)
            .expect("should generate ClientCert");

        let client_pem = client_crt.cert_pem().expect("pem");

        X509::from_pem(client_pem.as_bytes()).expect("ClientCert should give valid PEM");
        PrivateKey::private_key_from_pem(client_pem.as_bytes())
            .expect("ClientCert should have valid Private Key");

        // Verify that the CA issued the client cert
        match ca_cert.cert.issued(&client_crt.cert) {
            X509VerifyResult::OK => (),
            _ => panic!("CA did not issue this ClientCert"),
        }

        // read principal from client cert
        let (_, pem) = x509_parser::prelude::parse_x509_pem(client_pem.as_bytes()).expect("parse");
        let meta = CertMetadata::load_from(&pem.contents).expect("load");
        assert_eq!(meta.principal(), TEST_PRINCIPAL);
    }

    /*
    #[test]
    fn test_server_keypair() {
        let ca_cert = ClusterCA::generate(TEST_DOMAIN, TEST_HOSTNAME).unwrap();
        let (server_csr, server_key) =
            ServerCert::generate_csr(TEST_HOSTNAME, TEST_DOMAIN).unwrap();
        let server = ServerCert::generate(&ca_cert, &server_csr, server_key).unwrap();

        let message: &[u8] = b"Hello, world!";
        let mut encrypted = vec![0u8; 10000];
        let size = server
            .cert
            .public_key()
            .unwrap()
            .rsa()
            .unwrap()
            .public_encrypt(message, &mut encrypted, Padding::PKCS1)
            .unwrap();
        let encrypted = &encrypted[..size];
        println!("Encrypted blob: {:02X?}", &encrypted);

        let mut decrypted = vec![0u8; 10000];
        let size = server
            .key
            .rsa()
            .unwrap()
            .private_decrypt(encrypted, &mut decrypted, Padding::PKCS1)
            .unwrap();
        let decrypted = &decrypted[..size];
        assert_eq!(message, decrypted);
    }
    */
}
