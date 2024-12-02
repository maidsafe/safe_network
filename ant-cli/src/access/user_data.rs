// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::HashMap;

use autonomi::client::{
    address::{addr_to_str, str_to_addr},
    archive::ArchiveAddr,
    archive_private::PrivateArchiveAccess,
    registers::{RegisterAddress, RegisterSecretKey},
    vault::UserData,
};
use color_eyre::eyre::Result;

use super::{
    data_dir::get_client_data_dir_path,
    keys::{create_register_signing_key_file, get_register_signing_key},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct PrivateFileArchive {
    name: String,
    secret_access: String,
}

pub fn get_local_user_data() -> Result<UserData> {
    let register_sk = get_register_signing_key().map(|k| k.to_hex()).ok();
    let registers = get_local_registers()?;
    let file_archives = get_local_public_file_archives()?;
    let private_file_archives = get_local_private_file_archives()?;

    let user_data = UserData {
        register_sk,
        registers,
        file_archives,
        private_file_archives,
    };
    Ok(user_data)
}

pub fn get_local_private_file_archives() -> Result<HashMap<PrivateArchiveAccess, String>> {
    let data_dir = get_client_data_dir_path()?;
    let user_data_path = data_dir.join("user_data");
    let private_file_archives_path = user_data_path.join("private_file_archives");
    std::fs::create_dir_all(&private_file_archives_path)?;

    let mut private_file_archives = HashMap::new();
    for entry in walkdir::WalkDir::new(private_file_archives_path)
        .min_depth(1)
        .max_depth(1)
    {
        let entry = entry?;
        let file_content = std::fs::read_to_string(entry.path())?;
        let private_file_archive: PrivateFileArchive = serde_json::from_str(&file_content)?;
        let private_file_archive_access =
            PrivateArchiveAccess::from_hex(&private_file_archive.secret_access)?;
        private_file_archives.insert(private_file_archive_access, private_file_archive.name);
    }
    Ok(private_file_archives)
}

pub fn get_local_private_archive_access(local_addr: &str) -> Result<PrivateArchiveAccess> {
    let data_dir = get_client_data_dir_path()?;
    let user_data_path = data_dir.join("user_data");
    let private_file_archives_path = user_data_path.join("private_file_archives");
    let file_path = private_file_archives_path.join(local_addr);
    let file_content = std::fs::read_to_string(file_path)?;
    let private_file_archive: PrivateFileArchive = serde_json::from_str(&file_content)?;
    let private_file_archive_access =
        PrivateArchiveAccess::from_hex(&private_file_archive.secret_access)?;
    Ok(private_file_archive_access)
}

pub fn get_local_registers() -> Result<HashMap<RegisterAddress, String>> {
    let data_dir = get_client_data_dir_path()?;
    let user_data_path = data_dir.join("user_data");
    let registers_path = user_data_path.join("registers");
    std::fs::create_dir_all(&registers_path)?;

    let mut registers = HashMap::new();
    for entry in walkdir::WalkDir::new(registers_path)
        .min_depth(1)
        .max_depth(1)
    {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy();
        let register_address = RegisterAddress::from_hex(&file_name)?;
        let file_content = std::fs::read_to_string(entry.path())?;
        let register_name = file_content;
        registers.insert(register_address, register_name);
    }
    Ok(registers)
}

pub fn get_local_public_file_archives() -> Result<HashMap<ArchiveAddr, String>> {
    let data_dir = get_client_data_dir_path()?;
    let user_data_path = data_dir.join("user_data");
    let file_archives_path = user_data_path.join("file_archives");
    std::fs::create_dir_all(&file_archives_path)?;

    let mut file_archives = HashMap::new();
    for entry in walkdir::WalkDir::new(file_archives_path)
        .min_depth(1)
        .max_depth(1)
    {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy();
        let file_archive_address = str_to_addr(&file_name)?;
        let file_archive_name = std::fs::read_to_string(entry.path())?;
        file_archives.insert(file_archive_address, file_archive_name);
    }
    Ok(file_archives)
}

pub fn write_local_user_data(user_data: &UserData) -> Result<()> {
    if let Some(register_key) = &user_data.register_sk {
        let sk = RegisterSecretKey::from_hex(register_key)?;
        create_register_signing_key_file(sk)?;
    }

    for (register, name) in user_data.registers.iter() {
        write_local_register(register, name)?;
    }

    for (archive, name) in user_data.file_archives.iter() {
        write_local_public_file_archive(addr_to_str(*archive), name)?;
    }

    for (archive, name) in user_data.private_file_archives.iter() {
        write_local_private_file_archive(archive.to_hex(), archive.address(), name)?;
    }

    Ok(())
}

pub fn write_local_register(register: &RegisterAddress, name: &str) -> Result<()> {
    let data_dir = get_client_data_dir_path()?;
    let user_data_path = data_dir.join("user_data");
    let registers_path = user_data_path.join("registers");
    std::fs::create_dir_all(&registers_path)?;
    std::fs::write(registers_path.join(register.to_hex()), name)?;
    Ok(())
}

pub fn write_local_public_file_archive(archive: String, name: &str) -> Result<()> {
    let data_dir = get_client_data_dir_path()?;
    let user_data_path = data_dir.join("user_data");
    let file_archives_path = user_data_path.join("file_archives");
    std::fs::create_dir_all(&file_archives_path)?;
    std::fs::write(file_archives_path.join(archive), name)?;
    Ok(())
}

pub fn write_local_private_file_archive(
    archive: String,
    local_addr: String,
    name: &str,
) -> Result<()> {
    let data_dir = get_client_data_dir_path()?;
    let user_data_path = data_dir.join("user_data");
    let private_file_archives_path = user_data_path.join("private_file_archives");
    std::fs::create_dir_all(&private_file_archives_path)?;
    let file_name = local_addr;
    let content = serde_json::to_string(&PrivateFileArchive {
        name: name.to_string(),
        secret_access: archive,
    })?;
    std::fs::write(private_file_archives_path.join(file_name), content)?;
    Ok(())
}
