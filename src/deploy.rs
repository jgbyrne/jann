extern crate walkdir;

use std::path::{Path, PathBuf, Component};
use std::fs;
use std::convert;

// Deploy Options
// - Whether to overwrite {Files, Dirs} w/ {Files, Dirs}
// - Whether to create INTERmediary directories
#[derive(Clone, Copy, Debug)]
pub struct DepOpt {
    pub OW_FF: bool,
    pub OW_DD: bool,
    pub OW_FD: bool,
    pub OW_DF: bool,
    pub INTER: bool,
}

impl DepOpt {
    // check - Determine if an overwrite may take place based on these options
    fn check(&self, src_ent: &Entity, dst_ent: &Entity) -> bool {
        match *src_ent {
            Entity::FILE => {
                match *dst_ent {
                    Entity::FILE => {
                        self.OW_FF
                    },
                    Entity::DIR => {
                        self.OW_DF
                    }
                }
            },

            Entity::DIR => {
                match *dst_ent {
                    Entity::FILE => {
                        self.OW_FD
                    },
                    Entity::DIR => {
                        self.OW_DD
                    }
                }
            }
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum Entity {
    FILE,
    DIR,
}

// EndPtr - Points to last extant entity in a path
#[derive(Debug)]
struct EndPtr {
    ptr: usize,
    entity: Entity,
    full: bool,
}

// scout - Determine EndPtr for Destination path
fn scout(dst_cmps: &Vec<Component>) -> EndPtr {
    let mut scout_path = PathBuf::new();
    let mut entity = Entity::DIR;
    for (i, cmp) in dst_cmps.iter().enumerate() {
        scout_path.push(cmp);
        if scout_path.is_file() {
            entity = Entity::FILE;
        }
        else if scout_path.is_dir() {
            entity = Entity::DIR;
        }
        else {
            return EndPtr {
                ptr: i,
                entity,
                full: false,
            };
        }
    }
    EndPtr {
        ptr: dst_cmps.len(),
        entity, 
        full: true,
    }
}

#[derive(Debug)]
pub struct DeployError {
     pub source : String,
     pub message: String,
}

impl DeployError {
    fn locked(source: &'static str, message: &'static str) -> DeployError {
        DeployError { source: String::from(source), message: String::from(message) }
    }
}

impl convert::From<walkdir::Error> for DeployError {
    fn from(err: walkdir::Error) -> Self {
        DeployError {source : "WalkDir".to_owned(),
                     message: format!("{}", err)}
   }
}

impl convert::From<std::io::Error> for DeployError {
    fn from(err: std::io::Error) -> Self {
        DeployError {source : "IO".to_owned(),
                     message: format!("{}", err)}
    }
}

fn copy_dir<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<(), DeployError> {
    let src_path = src.as_ref();
    let dst_path = dst.as_ref();
     
    for entry in walkdir::WalkDir::new(src_path) {
        let entry = entry?;
        let path = entry.path();
        let linked = dst_path.join(path.strip_prefix(src_path).unwrap());
        if path.is_file() {
            fs::copy(path, &linked)?;
        }
        else {
            fs::create_dir_all(&linked)?;
        }
    }
    Ok(())
}


// Deploy from source to destination based on options
pub fn deploy(src: PathBuf, src_ent: Entity, dst: PathBuf, opt: DepOpt) -> Result<(), DeployError> {
    if option_env!("JANN_MOSTLY_HARMLESS") == Some("1") {
        println!("{:?} => {:?}\n...as {:?}\n... with {:?}", src, dst, src_ent, opt);
        return Ok(());
    }
    let src_cmps: Vec<Component> = src.components().collect();
    let dst_cmps: Vec<Component> = dst.components().collect();
    let dst_ptr = scout(&dst_cmps);

    if dst_ptr.full {
        let viable = opt.check(&src_ent, &dst_ptr.entity);
        if viable {
            match &dst_ptr.entity {
                Entity::FILE => {
                    fs::remove_file(&dst)?; //.expect("Could not remove destination file");
                },
                Entity::DIR => {
                    fs::remove_dir_all(&dst)?; //.expect("Could not remove destination directory");
                }
            }

            match &src_ent {
                Entity::FILE => {
                    fs::copy(&src, &dst)?;
                },
                Entity::DIR => {
                    copy_dir(&src, &dst)?;
                }
            }
        }
    }
    else {
        if dst_ptr.entity == Entity::FILE {
            if !opt.OW_FD { return Err(DeployError::locked("Deploy", "Options disallow overwriting files with directories.")) }

            let mut ow_path = PathBuf::new();
            for c in dst_cmps.iter().take(dst_ptr.ptr) {
                ow_path.push(c);
            }
            if ow_path.is_file() { // should always be true
                fs::remove_file(&ow_path)?; //.expect("Could not remove clashing file"); 
            }
            else {
                unreachable!();
            }
        }
        let parent = dst.parent().unwrap();
        if !parent.is_dir() {
            if opt.INTER {
                fs::create_dir_all(&parent)?;
            }
            else {
                return Err(DeployError::locked("Deploy", "Options disallow creating intermediate directories"));
            }
        }
        match &src_ent {
            Entity::FILE => {
                fs::copy(&src, &dst)?;
            },
            Entity::DIR => {
                fs::create_dir(&dst)?;
                copy_dir(&src, &dst)?;
            }
        }
    }
    Ok(())
}
