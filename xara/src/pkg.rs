use std::io::{Read, Seek, Write};

use crate::error::{XarError, Result};
use crate::toc::XarFileType;
use crate::XarArchive;

/// High-level reader for macOS .pkg (flat package) files.
/// Wraps a XarArchive with PKG-specific knowledge.
pub struct PkgReader<R: Read + Seek> {
    xar: XarArchive<R>,
}

impl<R: Read + Seek> PkgReader<R> {
    /// Open a .pkg file
    pub fn open(reader: R) -> Result<Self> {
        let xar = XarArchive::open(reader)?;
        Ok(PkgReader { xar })
    }

    /// Is this a product package (has Distribution XML)?
    pub fn is_product_package(&self) -> bool {
        self.xar.find("Distribution").is_some()
    }

    /// Get the Distribution XML (product packages only)
    pub fn distribution(&mut self) -> Result<Option<String>> {
        match self.xar.find("Distribution") {
            Some(file) => {
                let file = file.clone();
                let data = self.xar.read_file(&file)?;
                Ok(Some(String::from_utf8_lossy(&data).to_string()))
            }
            None => Ok(None),
        }
    }

    /// List component package names.
    /// For product packages, these are subdirectories like "foo.pkg".
    /// For component packages, returns a single empty string.
    pub fn components(&self) -> Vec<String> {
        let mut components = Vec::new();

        // Product packages have directories ending in .pkg at the root level
        for file in self.xar.files() {
            if file.parent.is_none()
                && file.file_type == XarFileType::Directory
                && file.name.ends_with(".pkg")
            {
                components.push(file.name.clone());
            }
        }

        // If no .pkg directories found, this is a component package
        if components.is_empty() {
            if self.xar.find("Payload").is_some() || self.xar.find("PackageInfo").is_some() {
                components.push(String::new());
            }
        }

        components
    }

    /// Get PackageInfo XML for a component
    pub fn package_info(&mut self, component: &str) -> Result<Option<String>> {
        let path = if component.is_empty() {
            "PackageInfo".to_string()
        } else {
            format!("{}/PackageInfo", component)
        };

        match self.xar.find(&path) {
            Some(file) => {
                let file = file.clone();
                let data = self.xar.read_file(&file)?;
                Ok(Some(String::from_utf8_lossy(&data).to_string()))
            }
            None => Ok(None),
        }
    }

    /// Extract Payload (PBZX data) for a component into memory
    pub fn payload(&mut self, component: &str) -> Result<Vec<u8>> {
        let path = if component.is_empty() {
            "Payload".to_string()
        } else {
            format!("{}/Payload", component)
        };

        match self.xar.find(&path) {
            Some(file) => {
                let file = file.clone();
                self.xar.read_file(&file)
            }
            None => Err(XarError::FileNotFound(path)),
        }
    }

    /// Stream Payload to a writer
    pub fn payload_to<W: Write>(&mut self, component: &str, writer: W) -> Result<u64> {
        let path = if component.is_empty() {
            "Payload".to_string()
        } else {
            format!("{}/Payload", component)
        };

        match self.xar.find(&path) {
            Some(file) => {
                let file = file.clone();
                self.xar.read_file_to(&file, writer)
            }
            None => Err(XarError::FileNotFound(path)),
        }
    }

    /// Access the underlying XAR archive
    pub fn xar(&self) -> &XarArchive<R> {
        &self.xar
    }

    /// Access the underlying XAR archive mutably
    pub fn xar_mut(&mut self) -> &mut XarArchive<R> {
        &mut self.xar
    }

    /// List all files in the archive
    pub fn list_files(&self) -> Vec<String> {
        self.xar.files().iter().map(|f| f.path.clone()).collect()
    }
}
