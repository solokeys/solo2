use littlefs2_core::DynFilesystem;

#[derive(Debug)]
pub struct Migrator {
    /// The function performing the migration
    ///
    /// First argument is the Internal Filesystem, second argument is the External
    pub migrate: fn(&dyn DynFilesystem, &dyn DynFilesystem) -> Result<(), littlefs2_core::Error>,

    /// The version of the storage for which the migration needs to be run
    pub version: u32,
}

#[cfg(feature = "migration-tests")]
pub mod test_utils {

    // Copyright (C) Nitrokey GmbH
    // SPDX-License-Identifier: Apache-2.0 or MIT

    use littlefs2::{
        fs::Filesystem, io::Error, object_safe::DynFilesystem, path, path::Path, ram_storage,
    };

    /// Represent a directory of data
    pub enum FsValues {
        Dir(&'static [(&'static Path, FsValues)]),
        File(usize),
    }

    type Result<T, E = Error> = core::result::Result<T, E>;

    /// Prepare the filesystem for a given tests values
    pub fn prepare_fs(fs: &dyn DynFilesystem, value: &FsValues, path: &Path) {
        match value {
            FsValues::File(f_data_len) => {
                fs.create_file_and_then(path, &mut |f| {
                    f.set_len(*f_data_len).unwrap();
                    Ok(())
                })
                .unwrap();
            }
            FsValues::Dir(d) => {
                if path != path!("/") {
                    fs.create_dir(path).unwrap();
                }
                for (p, v) in *d {
                    prepare_fs(fs, v, &path.join(p));
                }
            }
        }
    }

    /// Test equality between the filesystem and the expected values
    pub fn test_fs_equality(fs: &dyn DynFilesystem, value: &FsValues, path: &Path) {
        match value {
            FsValues::Dir(d) => {
                let mut expected_iter = d.iter();
                fs.read_dir_and_then(path, &mut |dir| {
                    // skip . and ..
                    dir.next().unwrap().unwrap();
                    dir.next().unwrap().unwrap();
                    for (expected_path, expected_values) in expected_iter.by_ref() {
                        let entry = dir.next().unwrap().unwrap();
                        assert_eq!(entry.file_name(), *expected_path);
                        test_fs_equality(fs, expected_values, &path.join(expected_path));
                    }
                    assert!(dir.next().is_none());
                    Ok(())
                })
                .unwrap();
            }
            FsValues::File(f_data_len) => {
                fs.open_file_and_then(path, &mut |f| {
                    let mut buf = [0; 512];
                    let data = f.read(&mut buf).unwrap();
                    assert_eq!(data, *f_data_len);
                    Ok(())
                })
                .unwrap();
            }
        }
    }

    ram_storage!(
        name = NoBackendStorage,
        backend = RamDirect,
        erase_value = 0xff,
        read_size = 16,
        write_size = 16,
        cache_size_ty = littlefs2::consts::U512,
        block_size = 512,
        block_count = 128,
        lookahead_size_ty = littlefs2::consts::U8,
        filename_max_plus_one_ty = littlefs2::consts::U256,
        path_max_plus_one_ty = littlefs2::consts::U256,
    );

    pub fn test_migration_one(
        before: &FsValues,
        after: &FsValues,
        migrate: impl Fn(&dyn DynFilesystem) -> Result<(), Error>,
    ) {
        test_migration(
            before,
            after,
            &FsValues::Dir(&[]),
            &FsValues::Dir(&[]),
            |ifs, _efs| migrate(ifs),
        );
    }

    pub fn test_migration(
        before_ifs: &FsValues,
        after_ifs: &FsValues,
        before_efs: &FsValues,
        after_efs: &FsValues,
        migrate: impl Fn(&dyn DynFilesystem, &dyn DynFilesystem) -> Result<(), Error>,
    ) {
        let mut storage_ifs = RamDirect::default();
        let mut storage_efs = RamDirect::default();

        let backend_efs = &mut NoBackendStorage::new(&mut storage_efs);
        let backend_ifs = &mut NoBackendStorage::new(&mut storage_ifs);

        Filesystem::format(backend_ifs).unwrap();
        Filesystem::format(backend_efs).unwrap();

        Filesystem::mount_and_then(backend_ifs, |ifs| {
            Filesystem::mount_and_then(backend_efs, |efs| {
                prepare_fs(ifs, before_ifs, path!("/"));
                prepare_fs(efs, before_efs, path!("/"));

                test_fs_equality(ifs, before_ifs, path!("/"));
                test_fs_equality(efs, before_efs, path!("/"));

                migrate(ifs, efs).unwrap();
                test_fs_equality(efs, after_efs, path!("/"));
                test_fs_equality(ifs, after_ifs, path!("/"));
                Ok(())
            })
            .unwrap();
            Ok(())
        })
        .unwrap();
    }
}
