use crate::domain::{Catalog, Ensure, FileResource, Resource, ResourceState, Transaction};
use crate::infrastructure::BackupStore;
use std::collections::HashMap;

pub struct RollbackEngine {
    backup_store: BackupStore,
}

impl RollbackEngine {
    pub fn new(backup_store: BackupStore) -> Self {
        Self { backup_store }
    }

    pub fn generate_rollback_catalog(&self, transaction: &Transaction) -> Catalog {
        let mut rollback_resources = Vec::new();

        // Rollback in reverse order of application
        let mut original_resources = transaction.original_catalog.resources.clone();
        original_resources.reverse();

        for resource in original_resources {
            if let Some(original_state) = transaction.original_states.get(resource.id()) {
                if let Some(rollback_res) =
                    self.invert_resource(&resource, original_state, &transaction.backups)
                {
                    rollback_resources.push(rollback_res);
                }
            }
        }

        Catalog::new(
            transaction.original_catalog.node_name.clone(),
            transaction.original_catalog.environment.clone(),
            rollback_resources,
        )
    }

    fn invert_resource(
        &self,
        resource: &Resource,
        original_state: &ResourceState,
        backups: &HashMap<String, String>,
    ) -> Option<Resource> {
        match resource {
            Resource::File(file) => {
                match original_state {
                    ResourceState::Ensure(Ensure::Absent) => {
                        // Was absent, now ensure absent again
                        Some(Resource::File(FileResource {
                            ensure: Ensure::Absent,
                            ..file.clone()
                        }))
                    }
                    ResourceState::Ensure(Ensure::Present)
                    | ResourceState::Full {
                        ensure: Ensure::Present,
                        ..
                    } => {
                        // Was present, need to restore content if we have it
                        let content = if let Some(hash) = backups.get(&file.id) {
                            self.backup_store
                                .retrieve(hash)
                                .ok()
                                .and_then(|c| String::from_utf8(c).ok())
                        } else if let ResourceState::Full {
                            content: Some(c), ..
                        } = original_state
                        {
                            String::from_utf8(c.clone()).ok()
                        } else {
                            None
                        };

                        Some(Resource::File(FileResource {
                            ensure: Ensure::Present,
                            content,
                            ..file.clone()
                        }))
                    }
                    _ => None,
                }
            }
            Resource::Directory(dir) => {
                let ensure = match original_state {
                    ResourceState::Ensure(e) => e.clone(),
                    ResourceState::Full { ensure, .. } => ensure.clone(),
                };
                Some(Resource::Directory(
                    crate::domain::resource::DirectoryResource {
                        ensure,
                        ..dir.clone()
                    },
                ))
            }
            Resource::Meta(_) => None,
        }
    }
}
