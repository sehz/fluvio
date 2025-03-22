pub(crate) mod follower;
pub(crate) mod leader;

#[cfg(test)]
pub(crate) mod test;

pub(crate) mod util {
    use fluvio::{Isolation, Offset};

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub struct OffsetInfo {
        pub hw: Offset,
        pub leo: Offset,
    }

    impl Default for OffsetInfo {
        fn default() -> Self {
            Self { hw: -1, leo: -1 }
        }
    }

    impl From<(Offset, Offset)> for OffsetInfo {
        fn from(value: (Offset, Offset)) -> Self {
            Self::new(value.0, value.1)
        }
    }

    impl OffsetInfo {
        pub fn new(leo: Offset, hw: Offset) -> Self {
            assert!(leo >= hw, "end offset >= high watermark");
            Self { hw, leo }
        }

        /// get isolation offset
        pub fn isolation(&self, isolation: &Isolation) -> Offset {
            match isolation {
                Isolation::ReadCommitted => self.hw,
                Isolation::ReadUncommitted => self.leo,
            }
        }

        /// check if offset contains valid value
        ///  invalid if either hw or leo is -1
        ///  or if hw > leo
        pub fn is_valid(&self) -> bool {
            !(self.hw == -1 || self.leo == -1) && self.leo >= self.hw
        }

        /// update hw, leo
        /// return true if there was change
        /// otherwise false
        pub fn update(&mut self, other: &Self) -> bool {
            let mut change = false;
            if other.hw > self.hw {
                self.hw = other.hw;
                change = true;
            }
            if other.leo > self.leo {
                self.leo = other.leo;
                change = true;
            }
            change
        }

        /// check if we are newer than other
        pub fn newer(&self, other: &Self) -> bool {
            self.leo > other.leo || self.hw > other.hw
        }

        pub fn is_same(&self, other: &Self) -> bool {
            self.hw == other.hw && self.leo == other.leo
        }

        /// is hw fully caught with leo
        pub fn is_committed(&self) -> bool {
            self.leo == self.hw
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_offset_validation() {
            assert!(!OffsetInfo::default().is_valid());

            assert!(!OffsetInfo { hw: 2, leo: 1 }.is_valid());

            assert!(OffsetInfo { hw: 2, leo: 3 }.is_valid());

            assert!(OffsetInfo { hw: 0, leo: 0 }.is_valid());

            assert!(OffsetInfo { hw: 4, leo: 4 }.is_valid());

            assert!(!OffsetInfo { hw: -1, leo: 3 }.is_valid());
        }

        #[test]
        fn test_offset_newer() {
            assert!(!OffsetInfo { hw: 1, leo: 2 }.newer(&OffsetInfo { hw: 2, leo: 2 }));

            assert!(OffsetInfo { hw: 2, leo: 10 }.newer(&OffsetInfo { hw: 0, leo: 0 }));
        }

        #[test]
        fn test_offset_update() {
            assert!(!OffsetInfo { hw: 1, leo: 2 }.update(&OffsetInfo { hw: 0, leo: 0 }));

            assert!(OffsetInfo { hw: 1, leo: 2 }.update(&OffsetInfo { hw: 1, leo: 3 }));
        }
    }
}
