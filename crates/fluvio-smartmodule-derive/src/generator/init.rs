use quote::quote;
use proc_macro2::TokenStream;
use crate::SmartModuleFn;

// generate init smartmodule
pub fn generate_init_smartmodule(func: &SmartModuleFn, has_params: bool) -> TokenStream {
    // let user_fn = &func.name;
    let user_code = func.func;

    let params_parsing = if has_params {
        quote!(
            use std::convert::TryInto;

            let params = match smartmodule_input.params.try_into(){
                Ok(params) => params,
                Err(err) => return SmartModuleInternalError::ParsingExtraParams as i32,
            };
        )
    } else {
        quote!()
    };

    /*
    let function_call = if has_params {
        quote!(
            super:: #user_fn(&record, &params)
        )
    } else {
        quote!(
            super:: #user_fn(&record)
        )
    };
    */

    quote! {
        #user_code

        mod __system {
            #[no_mangle]
            #[allow(clippy::missing_safety_doc)]
            pub unsafe fn filter(ptr: *mut u8, len: usize, version: i16) -> i32 {
                use fluvio_smartmodule::dataplane::smartmodule::{
                    SmartModuleInput, SmartModuleInternalError,
                    SmartModuleRuntimeError, SmartModuleKind, SmartModuleOutput,
                    SmartModuleExtraParams
                };
                use fluvio_smartmodule::dataplane::core::{Encoder, Decoder};
                use fluvio_smartmodule::dataplane::record::{Record, RecordData};

                // DECODING
                extern "C" {
                    fn copy_records(putr: i32, len: i32);
                }

                let input_data = Vec::from_raw_parts(ptr, len, len);
                let mut params = SmartModuleExtraParams::default();
                if let Err(_err) = Decoder::decode(&mut params, &mut std::io::Cursor::new(input_data), version) {
                    return SmartModuleInternalError::DecodingBaseInput as i32;
                }



                #params_parsing



                // ENCODING
                let mut out = vec![];
                if let Err(_) = Encoder::encode(&mut output, &mut out, version) {
                    return SmartModuleInternalError::EncodingOutput as i32;
                }

                let out_len = out.len();
                let ptr = out.as_mut_ptr();
                std::mem::forget(out);
                copy_records(ptr as i32, out_len as i32);
                output.successes.len() as i32
            }
        }
    }
}