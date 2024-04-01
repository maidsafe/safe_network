// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

// All tests require a network running so Clients can be instantiated.

use bls::SecretKey;
use eyre::Result;
use sn_client::test_utils::{
    get_funded_wallet, get_new_client, pay_for_storage, random_file_chunk,
};
use sn_client::{FolderEntry, FoldersApi, Metadata};
use sn_protocol::{storage::ChunkAddress, NetworkAddress};
use sn_registers::{EntryHash, RegisterAddress};
use xor_name::XorName;

#[tokio::test]
async fn test_folder_basics() -> Result<()> {
    let tmp_dir = tempfile::tempdir()?;
    let wallet_dir = tmp_dir.path();
    let mut rng = rand::thread_rng();
    let owner_sk = SecretKey::random();
    let owner_pk = owner_sk.public_key();
    let address = RegisterAddress::new(XorName::random(&mut rng), owner_pk);
    let address_subdir = RegisterAddress::new(XorName::random(&mut rng), owner_pk);
    let client = get_new_client(owner_sk).await?;
    let mut folders_api = FoldersApi::new(client, wallet_dir, Some(address))?;

    let file_chunk = random_file_chunk();

    let (file_entry_hash, file_meta_xorname, file_metadata) =
        folders_api.add_file("file.txt".into(), file_chunk.clone(), None)?;
    assert_eq!(
        file_metadata,
        Metadata {
            name: "file.txt".to_string(),
            content: FolderEntry::File(file_chunk)
        }
    );

    let (subdir_entry_hash, subdir_meta_xorname, subdir_metadata) =
        folders_api.add_folder("subdir".into(), address_subdir, None)?;
    assert_eq!(
        subdir_metadata,
        Metadata {
            name: "subdir".to_string(),
            content: FolderEntry::Folder(address_subdir)
        }
    );

    assert_eq!(folders_api.address(), &address);
    assert_eq!(
        folders_api.as_net_addr(),
        NetworkAddress::RegisterAddress(address)
    );
    assert_eq!(
        folders_api.meta_addrs_to_pay(),
        vec![
            NetworkAddress::ChunkAddress(ChunkAddress::new(file_meta_xorname)),
            NetworkAddress::ChunkAddress(ChunkAddress::new(subdir_meta_xorname))
        ]
        .into_iter()
        .collect()
    );

    assert!(folders_api.contains(&file_entry_hash));
    assert!(folders_api.contains(&subdir_entry_hash));
    assert!(!folders_api.contains(&EntryHash::default()));

    assert_eq!(
        folders_api.find_by_name("file.txt"),
        Some((&file_meta_xorname, &file_metadata))
    );
    assert_eq!(
        folders_api.find_by_name("subdir"),
        Some((&subdir_meta_xorname, &subdir_metadata))
    );
    assert!(folders_api.find_by_name("inexistent").is_none());

    assert_eq!(
        folders_api.entries().await?,
        vec![
            (file_entry_hash, (file_meta_xorname, file_metadata)),
            (subdir_entry_hash, (subdir_meta_xorname, subdir_metadata))
        ]
        .into_iter()
        .collect()
    );

    Ok(())
}

#[tokio::test]
async fn test_folder_remove_replace_entries() -> Result<()> {
    let tmp_dir = tempfile::tempdir()?;
    let wallet_dir = tmp_dir.path();
    let owner_sk = SecretKey::random();
    let client = get_new_client(owner_sk).await?;
    let mut folders_api = FoldersApi::new(client, wallet_dir, None)?;

    let file1_chunk = random_file_chunk();
    let file2_chunk = random_file_chunk();
    let file3_chunk = random_file_chunk();
    let file4_chunk = random_file_chunk();

    let (file1_entry_hash, _, _) =
        folders_api.add_file("file1.txt".into(), file1_chunk.clone(), None)?;
    let (file2_entry_hash, file2_meta_xorname, file2_metadata) =
        folders_api.add_file("file2.txt".into(), file2_chunk.clone(), None)?;

    assert_eq!(folders_api.entries().await?.len(), 2);
    assert!(folders_api.contains(&file1_entry_hash));
    assert!(folders_api.contains(&file2_entry_hash));
    assert!(folders_api.find_by_name("file1.txt").is_some());
    assert!(folders_api.find_by_name("file2.txt").is_some());

    // let's now test removing file1.txt
    folders_api.remove_item(file1_entry_hash)?;
    assert!(!folders_api.contains(&file1_entry_hash));
    assert!(folders_api.contains(&file2_entry_hash));
    assert!(folders_api.find_by_name("file1.txt").is_none());
    assert_eq!(
        folders_api.find_by_name("file2.txt"),
        Some((&file2_meta_xorname, &file2_metadata))
    );
    assert_eq!(
        folders_api.entries().await?,
        vec![(file2_entry_hash, (file2_meta_xorname, file2_metadata)),]
            .into_iter()
            .collect()
    );

    // now we test replacing file2.txt with file3.txt
    let (file3_entry_hash, file3_meta_xorname, file3_metadata) =
        folders_api.replace_file(file2_entry_hash, "file3.txt".into(), file3_chunk, None)?;
    assert!(!folders_api.contains(&file2_entry_hash));
    assert!(folders_api.contains(&file3_entry_hash));
    assert!(folders_api.find_by_name("file1.txt").is_none());
    assert!(folders_api.find_by_name("file2.txt").is_none());
    assert_eq!(
        folders_api.find_by_name("file3.txt"),
        Some((&file3_meta_xorname, &file3_metadata))
    );
    assert_eq!(
        folders_api.entries().await?,
        vec![(
            file3_entry_hash,
            (file3_meta_xorname, file3_metadata.clone())
        ),]
        .into_iter()
        .collect()
    );

    // let's add file4.txt, and check that final state is correct
    let (file4_entry_hash, file4_meta_xorname, file4_metadata) =
        folders_api.add_file("file4.txt".into(), file4_chunk, None)?;

    assert!(!folders_api.contains(&file1_entry_hash));
    assert!(!folders_api.contains(&file2_entry_hash));
    assert!(folders_api.contains(&file3_entry_hash));
    assert!(folders_api.contains(&file4_entry_hash));

    assert!(folders_api.find_by_name("file1.txt").is_none());
    assert!(folders_api.find_by_name("file2.txt").is_none());
    assert_eq!(
        folders_api.find_by_name("file3.txt"),
        Some((&file3_meta_xorname, &file3_metadata))
    );
    assert_eq!(
        folders_api.find_by_name("file4.txt"),
        Some((&file4_meta_xorname, &file4_metadata))
    );

    assert_eq!(
        folders_api.entries().await?,
        vec![
            (file3_entry_hash, (file3_meta_xorname, file3_metadata)),
            (file4_entry_hash, (file4_meta_xorname, file4_metadata))
        ]
        .into_iter()
        .collect()
    );

    Ok(())
}

#[tokio::test]
async fn test_folder_retrieve() -> Result<()> {
    let owner_sk = SecretKey::random();
    let client = get_new_client(owner_sk).await?;
    let tmp_dir = tempfile::tempdir()?;
    let wallet_dir = tmp_dir.path();
    let _ = get_funded_wallet(&client, wallet_dir).await?;

    let mut folder = FoldersApi::new(client.clone(), wallet_dir, None)?;
    let mut subfolder = FoldersApi::new(client.clone(), wallet_dir, None)?;

    let file1_chunk = random_file_chunk();

    let (file1_entry_hash, file1_meta_xorname, file1_metadata) =
        folder.add_file("file1.txt".into(), file1_chunk.clone(), None)?;
    let (subfolder_entry_hash, subfolder_meta_xorname, subfolder_metadata) =
        folder.add_folder("subfolder".into(), *subfolder.address(), None)?;

    let file2_chunk = random_file_chunk();
    let (file2_entry_hash, file2_meta_xorname, file2_metadata) =
        subfolder.add_file("file2.txt".into(), file2_chunk.clone(), None)?;

    // let's pay for storage
    let mut addrs2pay = vec![folder.as_net_addr(), subfolder.as_net_addr()];
    addrs2pay.extend(folder.meta_addrs_to_pay());
    addrs2pay.extend(subfolder.meta_addrs_to_pay());
    pay_for_storage(&client, wallet_dir, addrs2pay).await?;

    folder.sync(Default::default()).await?;
    subfolder.sync(Default::default()).await?;

    let mut retrieved_folder =
        FoldersApi::retrieve(client.clone(), wallet_dir, *folder.address()).await?;
    let mut retrieved_subfolder =
        FoldersApi::retrieve(client, wallet_dir, *subfolder.address()).await?;

    assert_eq!(retrieved_folder.entries().await?.len(), 2);
    assert!(retrieved_folder.contains(&file1_entry_hash));
    assert!(retrieved_folder.contains(&subfolder_entry_hash));
    assert_eq!(
        retrieved_folder.find_by_name("file1.txt"),
        Some((&file1_meta_xorname, &file1_metadata))
    );
    assert_eq!(
        retrieved_folder.find_by_name("subfolder"),
        Some((&subfolder_meta_xorname, &subfolder_metadata))
    );

    assert_eq!(retrieved_subfolder.entries().await?.len(), 1);
    assert!(retrieved_subfolder.contains(&file2_entry_hash));
    assert_eq!(
        retrieved_subfolder.find_by_name("file2.txt"),
        Some((&file2_meta_xorname, &file2_metadata))
    );

    assert_eq!(
        retrieved_folder.entries().await?,
        vec![
            (file1_entry_hash, (file1_meta_xorname, file1_metadata)),
            (
                subfolder_entry_hash,
                (subfolder_meta_xorname, subfolder_metadata)
            ),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        retrieved_subfolder.entries().await?,
        vec![(file2_entry_hash, (file2_meta_xorname, file2_metadata)),]
            .into_iter()
            .collect()
    );

    Ok(())
}

#[tokio::test]
async fn test_folder_merge_changes() -> Result<()> {
    let owner_sk = SecretKey::random();
    let client = get_new_client(owner_sk.clone()).await?;
    let tmp_dir = tempfile::tempdir()?;
    let wallet_dir = tmp_dir.path();
    let _ = get_funded_wallet(&client, wallet_dir).await?;

    let mut rng = rand::thread_rng();
    let owner_pk = owner_sk.public_key();
    let folder_addr = RegisterAddress::new(XorName::random(&mut rng), owner_pk);
    let subfolder_addr = RegisterAddress::new(XorName::random(&mut rng), owner_pk);

    let mut folder_a = FoldersApi::new(client.clone(), wallet_dir, Some(folder_addr))?;
    let mut subfolder_a = FoldersApi::new(client.clone(), wallet_dir, Some(subfolder_addr))?;
    let file_a1_chunk = random_file_chunk();
    let file_a2_chunk = random_file_chunk();

    let (file_a1_entry_hash, file_a1_meta_xorname, file_a1_metadata) =
        folder_a.add_file("fileA1.txt".into(), file_a1_chunk.clone(), None)?;
    let (subfolder_a_entry_hash, subfolder_a_meta_xorname, subfolder_a_metadata) =
        folder_a.add_folder("subfolderA".into(), *subfolder_a.address(), None)?;
    let (file_a2_entry_hash, file_a2_meta_xorname, file_a2_metadata) =
        subfolder_a.add_file("fileA2.txt".into(), file_a2_chunk.clone(), None)?;

    let mut folder_b = FoldersApi::new(client.clone(), wallet_dir, Some(folder_addr))?;
    let mut subfolder_b = FoldersApi::new(client.clone(), wallet_dir, Some(subfolder_addr))?;
    let file_b1_chunk = random_file_chunk();
    let file_b2_chunk = random_file_chunk();

    let (file_b1_entry_hash, file_b1_meta_xorname, file_b1_metadata) =
        folder_b.add_file("fileB1.txt".into(), file_b1_chunk.clone(), None)?;
    let (subfolder_b_entry_hash, subfolder_b_meta_xorname, subfolder_b_metadata) =
        folder_b.add_folder("subfolderB".into(), *subfolder_b.address(), None)?;
    let (file_b2_entry_hash, file_b2_meta_xorname, file_b2_metadata) =
        subfolder_b.add_file("fileB2.txt".into(), file_b2_chunk.clone(), None)?;

    // let's pay for storage
    let mut addrs2pay = vec![folder_a.as_net_addr(), subfolder_a.as_net_addr()];
    addrs2pay.extend(folder_a.meta_addrs_to_pay());
    addrs2pay.extend(subfolder_a.meta_addrs_to_pay());
    addrs2pay.extend(folder_b.meta_addrs_to_pay());
    addrs2pay.extend(subfolder_b.meta_addrs_to_pay());
    pay_for_storage(&client, wallet_dir, addrs2pay).await?;

    folder_a.sync(Default::default()).await?;
    subfolder_a.sync(Default::default()).await?;
    folder_b.sync(Default::default()).await?;
    subfolder_b.sync(Default::default()).await?;
    folder_a.sync(Default::default()).await?;
    subfolder_a.sync(Default::default()).await?;

    let folder_a_entries = folder_a.entries().await?;
    let folder_b_entries = folder_b.entries().await?;
    let subfolder_a_entries = subfolder_a.entries().await?;
    let subfolder_b_entries = subfolder_b.entries().await?;

    assert_eq!(folder_a_entries.len(), 4);
    assert_eq!(folder_b_entries.len(), 4);
    assert_eq!(subfolder_a_entries.len(), 2);
    assert_eq!(subfolder_b_entries.len(), 2);

    assert!(folder_a.contains(&file_a1_entry_hash));
    assert!(folder_a.contains(&file_b1_entry_hash));
    assert!(folder_a.contains(&subfolder_a_entry_hash));
    assert!(folder_a.contains(&subfolder_b_entry_hash));
    assert!(subfolder_a.contains(&file_a2_entry_hash));
    assert!(subfolder_a.contains(&file_b2_entry_hash));

    assert!(folder_b.contains(&file_a1_entry_hash));
    assert!(folder_b.contains(&file_b1_entry_hash));
    assert!(folder_b.contains(&subfolder_a_entry_hash));
    assert!(folder_b.contains(&subfolder_b_entry_hash));
    assert!(subfolder_b.contains(&file_a2_entry_hash));
    assert!(subfolder_b.contains(&file_b2_entry_hash));

    assert_eq!(
        folder_a.find_by_name("fileA1.txt"),
        Some((&file_a1_meta_xorname, &file_a1_metadata))
    );
    assert_eq!(
        folder_a.find_by_name("fileB1.txt"),
        Some((&file_b1_meta_xorname, &file_b1_metadata))
    );
    assert_eq!(
        folder_a.find_by_name("subfolderA"),
        Some((&subfolder_a_meta_xorname, &subfolder_a_metadata))
    );
    assert_eq!(
        folder_a.find_by_name("subfolderB"),
        Some((&subfolder_b_meta_xorname, &subfolder_b_metadata))
    );

    assert_eq!(
        folder_b.find_by_name("fileA1.txt"),
        Some((&file_a1_meta_xorname, &file_a1_metadata))
    );
    assert_eq!(
        folder_b.find_by_name("fileB1.txt"),
        Some((&file_b1_meta_xorname, &file_b1_metadata))
    );
    assert_eq!(
        folder_b.find_by_name("subfolderA"),
        Some((&subfolder_a_meta_xorname, &subfolder_a_metadata))
    );
    assert_eq!(
        folder_b.find_by_name("subfolderB"),
        Some((&subfolder_b_meta_xorname, &subfolder_b_metadata))
    );

    assert_eq!(folder_a_entries, folder_b_entries);
    assert_eq!(
        folder_a_entries,
        vec![
            (file_a1_entry_hash, (file_a1_meta_xorname, file_a1_metadata)),
            (file_b1_entry_hash, (file_b1_meta_xorname, file_b1_metadata)),
            (
                subfolder_a_entry_hash,
                (subfolder_a_meta_xorname, subfolder_a_metadata)
            ),
            (
                subfolder_b_entry_hash,
                (subfolder_b_meta_xorname, subfolder_b_metadata)
            ),
        ]
        .into_iter()
        .collect()
    );

    assert_eq!(
        subfolder_a.find_by_name("fileA2.txt"),
        Some((&file_a2_meta_xorname, &file_a2_metadata))
    );
    assert_eq!(
        subfolder_a.find_by_name("fileB2.txt"),
        Some((&file_b2_meta_xorname, &file_b2_metadata))
    );

    assert_eq!(subfolder_a_entries, subfolder_b_entries);
    assert_eq!(
        subfolder_a_entries,
        vec![
            (file_a2_entry_hash, (file_a2_meta_xorname, file_a2_metadata)),
            (file_b2_entry_hash, (file_b2_meta_xorname, file_b2_metadata))
        ]
        .into_iter()
        .collect()
    );

    Ok(())
}
