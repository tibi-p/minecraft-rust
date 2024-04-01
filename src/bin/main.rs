use std::env;
use std::fs::File;
use std::io;
use std::io::Read;

#[derive(Clone, Debug, PartialEq)]
enum TagType {
    End,
    Byte,
    Int32,
    Int64,
    Float,
    String,
    List,
    Compound,
}

impl TagType {
    fn parse<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut type_buf = [0; 1];
        reader.read_exact(&mut type_buf)?;
        let tag_type_byte = type_buf[0];
        let tag_type = match tag_type_byte {
            0 => TagType::End,
            1 => TagType::Byte,
            3 => TagType::Int32,
            4 => TagType::Int64,
            5 => TagType::Float,
            8 => TagType::String,
            9 => TagType::List,
            10 => TagType::Compound,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid tag type: {}", tag_type_byte))),
        };
        Ok(tag_type)
	}
}

#[derive(Debug)]
enum Choice {
    Byte(u8),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    String(String),
    List(TagType, Vec<Choice>),
    Vec(Vec<Tag>),
}

impl Choice {
    fn parse<R: Read>(reader: &mut R, tag_type: TagType) -> io::Result<Self> {
        match tag_type {
            TagType::End => Err(io::Error::new(io::ErrorKind::InvalidData, "Cannot parse value of End tag")),
            TagType::Byte => {
                let mut byte_value_buf = [0; 1];
                reader.read_exact(&mut byte_value_buf)?;
                Ok(Choice::Byte(byte_value_buf[0]))
            }
            TagType::Int32 => {
                let mut int32_value_buf = [0; 4];
                reader.read_exact(&mut int32_value_buf)?;
                Ok(Choice::Int32(i32::from_le_bytes(int32_value_buf)))
            }
            TagType::Int64 => {
                let mut int64_value_buf = [0; 8];
                reader.read_exact(&mut int64_value_buf)?;
                Ok(Choice::Int64(i64::from_le_bytes(int64_value_buf)))
            }
            TagType::Float => {
                let mut float_value_buf = [0; 4];
                reader.read_exact(&mut float_value_buf)?;
                let float_value = f32::from_le_bytes(float_value_buf);
                Ok(Choice::Float32(float_value))
            }
            TagType::String => {
                let mut length_buf = [0; 2];
                reader.read_exact(&mut length_buf)?;
                let length = u16::from_le_bytes(length_buf) as usize;
                let mut string_value_buf = vec![0; length];
                reader.read_exact(&mut string_value_buf)?;
                Ok(Choice::String(String::from_utf8_lossy(&string_value_buf).into_owned()))
            }
            TagType::List => {
                let element_type = TagType::parse(reader)?;
                let mut length_buf = [0; 4];
                reader.read_exact(&mut length_buf)?;
                let length = u32::from_le_bytes(length_buf) as usize;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    let element = Self::parse(reader, element_type.clone())?;
                    values.push(element);
                }
                Ok(Choice::List(element_type, values))
            }
            TagType::Compound => {
                let mut compound_tags = Vec::new();
                loop {
                    match Tag::parse(reader) {
                        Ok(child_tag) => {
                            if child_tag.tag_type == TagType::End {
                                break;
                            }
                            compound_tags.push(child_tag);
                        }
                        Err(err) => {
                            eprintln!("Error parsing child tag: {}", err);
                            return Err(err);
                        }
                    }
                }
                Ok(Choice::Vec(compound_tags))
            }
        }
    }
}

#[derive(Debug)]
struct Tag {
    tag_type: TagType,
    key: String,
    choice_value: Option<Choice>,
}

impl Tag {
    fn typed_parse<R: Read>(reader: &mut R, key: String, tag_type: TagType) -> io::Result<Self> {
        let mut tag = Tag {
            tag_type: tag_type.clone(),
            key,
            choice_value: Some(Choice::parse(reader, tag_type)?),
        };

        //println!("{} ({:?}): {:?}", tag.key, tag_type, tag.choice_value);

        Ok(tag)
    }

    fn parse<R: Read>(reader: &mut R) -> io::Result<Self> {
        let tag_type = TagType::parse(reader)?;

        if tag_type == TagType::End {
            return Ok(Tag {
                tag_type,
                key: "".to_string(),
                choice_value: None,
            });
        }

        let mut key_length_buf = [0; 2];
        reader.read_exact(&mut key_length_buf)?;
        let key_length = u16::from_le_bytes(key_length_buf) as usize;

        let mut key_buf = vec![0; key_length];
        reader.read_exact(&mut key_buf)?;
        let key = String::from_utf8_lossy(&key_buf).into_owned();

        Self::typed_parse(reader, key, tag_type)
    }
}

#[derive(Debug)]
struct LevelData {
    version: i32,
    buffer_length: i32,
    tags: Vec<Tag>
}

impl LevelData {
    fn from_file(world_dir: &str) -> io::Result<Self> {
        // Construct file path
        let file_path = format!("{}/level.dat", world_dir);

        // Open the file in read-only mode
        let mut file = File::open(&file_path)?;

        // Read the version
        let mut version_buffer = [0; 4];
        file.read_exact(&mut version_buffer)?;
        let version = i32::from_le_bytes(version_buffer);

        // Read the buffer length
        let mut buffer_length_buffer = [0; 4];
        file.read_exact(&mut buffer_length_buffer)?;
        let buffer_length = i32::from_le_bytes(buffer_length_buffer);

        // Read the buffer
        let mut tags = Vec::new();
        while let Ok(tag) = Tag::parse(&mut file) {
            if tag.tag_type == TagType::End {
                break;
            }
            tags.push(tag);
        }

        Ok(LevelData {
            version,
            buffer_length,
            tags,
        })
    }

    fn print(&self) {
        println!("Version: {}", self.version);
        println!("Buffer Length: {}", self.buffer_length);
        println!("Tags: {:?}", self.tags);
    }
}

fn main() -> io::Result<()> {
	// Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    // Check if --world_dir argument is provided
    if args.len() < 3 {
        eprintln!("Usage: {} --world_dir <world_directory>", args[0]);
        std::process::exit(1);
    }

    // Extract world directory from command-line arguments
    let world_dir = &args[2];

    // Read level data from the file
    let level_data = LevelData::from_file(world_dir)?;

    // Print the level data
    println!("Level Data:");
    level_data.print();

    Ok(())
}
