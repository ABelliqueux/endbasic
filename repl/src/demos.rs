// EndBASIC
// Copyright 2021 Julio Merino
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License.  You may obtain a copy
// of the License at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
// License for the specific language governing permissions and limitations
// under the License.

//! Exposes EndBASIC demos as a read-only drive.

use async_trait::async_trait;
use endbasic_std::storage::{DiskSpace, Drive, DriveFactory, DriveFiles, Metadata};
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::str;

/// A read-only drive that exposes a bunch of read-only demo files.
pub struct DemosDrive {
    /// The demos to expose, expressed as a mapping of names to (metadata, content) pairs.
    demos: HashMap<&'static str, (Metadata, String)>,
}

/// Converts the raw bytes of a demo file into the program string to expose.
///
/// The input `bytes` must be valid UTF-8, which we can guarantee because these bytes come from
/// files that we own in the source tree.
///
/// On Windows, the output string has all CRLF pairs converted to LF to ensure that the reported
/// demo file sizes are consistent across platforms.
fn process_demo(bytes: &[u8]) -> String {
    let raw_content = str::from_utf8(bytes).expect("Malformed demo file");
    if cfg!(target_os = "windows") {
        raw_content.replace("\r\n", "\n")
    } else {
        raw_content.to_owned()
    }
}

impl Default for DemosDrive {
    /// Creates a new demo drive.
    fn default() -> Self {
        let mut demos = HashMap::default();
        {
            let content = process_demo(include_bytes!("../examples/fibonacci.bas"));
            let metadata = Metadata {
                date: time::OffsetDateTime::from_unix_timestamp(1719672741).unwrap(),
                length: content.len() as u64,
            };
            demos.insert("FIBONACCI.BAS", (metadata, content));
        }
        {
            let content = process_demo(include_bytes!("../examples/guess.bas"));
            let metadata = Metadata {
                date: time::OffsetDateTime::from_unix_timestamp(1608693152).unwrap(),
                length: content.len() as u64,
            };
            demos.insert("GUESS.BAS", (metadata, content));
        }
        {
            let content = process_demo(include_bytes!("../examples/gpio.bas"));
            let metadata = Metadata {
                date: time::OffsetDateTime::from_unix_timestamp(1613316558).unwrap(),
                length: content.len() as u64,
            };
            demos.insert("GPIO.BAS", (metadata, content));
        }
        {
            let content = process_demo(include_bytes!("../examples/hello.bas"));
            let metadata = Metadata {
                date: time::OffsetDateTime::from_unix_timestamp(1608646800).unwrap(),
                length: content.len() as u64,
            };
            demos.insert("HELLO.BAS", (metadata, content));
        }
        {
            let content = process_demo(include_bytes!("../examples/palette.bas"));
            let metadata = Metadata {
                date: time::OffsetDateTime::from_unix_timestamp(1671243940).unwrap(),
                length: content.len() as u64,
            };
            demos.insert("PALETTE.BAS", (metadata, content));
        }
        {
            let content = process_demo(include_bytes!("../examples/tour.bas"));
            let metadata = Metadata {
                date: time::OffsetDateTime::from_unix_timestamp(1608774770).unwrap(),
                length: content.len() as u64,
            };
            demos.insert("TOUR.BAS", (metadata, content));
        }
        Self { demos }
    }
}

#[async_trait(?Send)]
impl Drive for DemosDrive {
    async fn delete(&mut self, _name: &str) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "The demos drive is read-only"))
    }

    async fn enumerate(&self) -> io::Result<DriveFiles> {
        let mut entries = BTreeMap::new();
        let mut bytes = 0;
        for (name, (metadata, content)) in self.demos.iter() {
            entries.insert(name.to_string(), metadata.clone());
            bytes += content.len();
        }
        let files = self.demos.len();

        let disk_quota = if bytes <= u64::MAX as usize && files <= u64::MAX as usize {
            Some(DiskSpace::new(bytes as u64, files as u64))
        } else {
            // Cannot represent the amount of disk within a DiskSpace.
            None
        };
        let disk_free = Some(DiskSpace::new(0, 0));

        Ok(DriveFiles::new(entries, disk_quota, disk_free))
    }

    async fn get(&self, name: &str) -> io::Result<Vec<u8>> {
        let uc_name = name.to_ascii_uppercase();
        match self.demos.get(&uc_name.as_ref()) {
            Some(value) => {
                let (_metadata, content) = value;
                Ok(content.as_bytes().to_owned())
            }
            None => Err(io::Error::new(io::ErrorKind::NotFound, "Demo not found")),
        }
    }

    async fn put(&mut self, _name: &str, _content: &[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "The demos drive is read-only"))
    }
}

/// Factory for demo drives.
#[derive(Default)]
pub struct DemoDriveFactory {}

impl DriveFactory for DemoDriveFactory {
    fn create(&self, target: &str) -> io::Result<Box<dyn Drive>> {
        if target.is_empty() {
            Ok(Box::from(DemosDrive::default()))
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot specify a path to mount a demos drive",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::block_on;

    #[test]
    fn test_demos_drive_delete() {
        let mut drive = DemosDrive::default();

        assert_eq!(
            io::ErrorKind::PermissionDenied,
            block_on(drive.delete("hello.bas")).unwrap_err().kind()
        );
        assert_eq!(
            io::ErrorKind::PermissionDenied,
            block_on(drive.delete("Hello.BAS")).unwrap_err().kind()
        );

        assert_eq!(
            io::ErrorKind::PermissionDenied,
            block_on(drive.delete("unknown.bas")).unwrap_err().kind()
        );
    }

    #[test]
    fn test_demos_drive_enumerate() {
        let drive = DemosDrive::default();

        let files = block_on(drive.enumerate()).unwrap();
        assert!(files.dirents().contains_key("FIBONACCI.BAS"));
        assert!(files.dirents().contains_key("GPIO.BAS"));
        assert!(files.dirents().contains_key("GUESS.BAS"));
        assert!(files.dirents().contains_key("HELLO.BAS"));
        assert!(files.dirents().contains_key("PALETTE.BAS"));
        assert!(files.dirents().contains_key("TOUR.BAS"));

        assert!(files.disk_quota().unwrap().bytes() > 0);
        assert_eq!(6, files.disk_quota().unwrap().files());
        assert_eq!(DiskSpace::new(0, 0), files.disk_free().unwrap());
    }

    #[test]
    fn test_demos_drive_get() {
        let drive = DemosDrive::default();

        assert_eq!(io::ErrorKind::NotFound, block_on(drive.get("unknown.bas")).unwrap_err().kind());

        assert_eq!(
            process_demo(include_bytes!("../examples/hello.bas")).as_bytes(),
            block_on(drive.get("hello.bas")).unwrap().as_slice()
        );
        assert_eq!(
            process_demo(include_bytes!("../examples/hello.bas")).as_bytes(),
            block_on(drive.get("Hello.Bas")).unwrap().as_slice()
        );
    }

    #[test]
    fn test_demos_drive_put() {
        let mut drive = DemosDrive::default();

        assert_eq!(
            io::ErrorKind::PermissionDenied,
            block_on(drive.put("hello.bas", b"")).unwrap_err().kind()
        );
        assert_eq!(
            io::ErrorKind::PermissionDenied,
            block_on(drive.put("Hello.BAS", b"")).unwrap_err().kind()
        );

        assert_eq!(
            io::ErrorKind::PermissionDenied,
            block_on(drive.put("unknown.bas", b"")).unwrap_err().kind()
        );
    }

    #[test]
    fn test_demos_drive_system_path() {
        let drive = DemosDrive::default();
        assert!(drive.system_path("foo").is_none());
    }
}
