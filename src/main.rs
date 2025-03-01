mod filesystem;
mod formats;
mod rom;

use crate::formats::portrait::KaoFile;

use filesystem::{FileAllocationTable, FileNameTable};
use rom::read_header;
use std::fs;
use std::path::PathBuf;

use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let rom_path = PathBuf::from("../../ROMs/pmd_eos_us.nds");
    let output_dir = PathBuf::from("./output/FONT");

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        if let Err(e) = fs::create_dir_all(&output_dir) {
            eprintln!("Failed to create output directory: {}", e);
            return;
        }
    }

    println!("ROM Path: {:?}", &rom_path);
    println!("Output Dir: {:?}", output_dir);

    let header = read_header(&rom_path);
    println!("ROM Header: {:#?}", header);

    let rom_data = fs::read(&rom_path).unwrap();

    let header = read_header(&rom_path);

    // Maybe change this to take a function to take a ROM header instead?
    let fat =
        FileAllocationTable::read_from_rom(&rom_data, header.fat_offset, header.fat_size).unwrap();

    let fnt = FileNameTable::read_from_rom(&rom_data, header.fnt_offset).unwrap();

    let kao_id = fnt
        .get_file_id("FONT/kaomado.kao")
        .ok_or("Could not find kaomado.kao")
        .unwrap();

    let kao_data = fat
        .get_file_data(kao_id as usize, &rom_data)
        .ok_or("Could not read KAO file data")
        .unwrap();

    println!("Found KAO file: FONT/kaomado.kao (ID: {})", kao_id);
    println!("KAO file size: {} bytes", kao_data.len());

    let kao_file = KaoFile::from_bytes(kao_data.to_vec()).unwrap();

    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))
        .unwrap();

    //    match extract_portrait_pokedex(kao_file, &output_dir) {
    //        Ok(_) => println!("Successfully extracted portraits!"),
    //        Err(e) => eprintln!("Error extracting portraits: {}", e),
    //    }

    match extract_portrait_expressions(kao_file, &output_dir) {
        Ok(_) => println!("Successfully extracted portraits!"),
        Err(e) => eprintln!("Error extracting portraits: {}", e),
    }
}

fn extract_portrait_pokedex(kao_file: KaoFile, output_dir: &PathBuf) -> Result<(), String> {
    let num_threads = thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);

    println!("Using {} threads for extraction", num_threads);

    let max_pokemon_id = 551;

    let chunk_size = (max_pokemon_id + 1) / num_threads;

    println!("Extracting portraits to build sprite atlases...");

    let kao_file = Arc::new(kao_file);
    let output_dir = Arc::new(output_dir.clone());
    let extracted_count = Arc::new(Mutex::new(0));

    let mut handles = Vec::new();

    for thread_id in 0..num_threads {
        let start_id = thread_id * chunk_size;
        let end_id = if thread_id == num_threads - 1 {
            max_pokemon_id
        } else {
            (thread_id + 1) * chunk_size - 1
        };

        let kao_file_clone = Arc::clone(&kao_file);
        let output_dir_clone = Arc::clone(&output_dir);
        let count_clone = Arc::clone(&extracted_count);

        let handle = thread::spawn(move || {
            let mut local_count = 0;

            for pokemon_id in start_id..=end_id {
                // Skip filler portraits
                if pokemon_id < 551 && pokemon_id > 535 {
                    continue;
                }

                if let Ok(Some(portrait)) = kao_file_clone.get_portrait(pokemon_id, 0) {
                    let output_path =
                        output_dir_clone.join(format!("pokemon_{:03}.png", pokemon_id + 1));

                    match portrait.to_rgba_image() {
                        Ok(image) => {
                            if let Err(e) = image.save(&output_path) {
                                println!("Warning: Failed to save portrait {} {}", pokemon_id, e);
                            } else {
                                local_count += 1;
                            }
                        }
                        Err(e) => {
                            println!(
                                "Warning: Failed to convert portrait {} to image: {}",
                                pokemon_id, e
                            );
                        }
                    }
                }
            }

            let mut global_count = count_clone.lock().unwrap();
            *global_count += local_count;
            println!("Thread {} extracted {} portraits", thread_id, local_count);
        });

        handles.push(handle);
    }

    for handle in handles {
        if let Err(e) = handle.join() {
            println!("Warning: Thread panicked: {:?}", e);
        }
    }

    let final_count = *extracted_count.lock().unwrap();
    println!("Successfully extracted {} portraits", final_count);
    Ok(())
}

fn extract_portrait_expressions(kao_file: KaoFile, output_dir: &PathBuf) -> Result<(), String> {
    let num_threads = thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);

    println!("Using {} threads for extraction", num_threads);

    let max_pokemon_id = 535;
    let chunk_size = (max_pokemon_id + 1) / num_threads;

    println!("Extracting portraits to build sprite atlases...");

    let kao_file = Arc::new(kao_file);
    let output_dir = Arc::new(output_dir.clone());
    let extracted_count = Arc::new(Mutex::new(0));

    let mut handles = Vec::new();

    for thread_id in 0..num_threads {
        let start_id = thread_id * chunk_size;
        let end_id = if thread_id == num_threads - 1 {
            max_pokemon_id
        } else {
            (thread_id + 1) * chunk_size - 1
        };

        let kao_file_clone = Arc::clone(&kao_file);
        let output_dir_clone = Arc::clone(&output_dir);
        let count_clone = Arc::clone(&extracted_count);

        let handle = thread::spawn(move || {
            let mut local_count = 0;

            let emotion_indices = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 32, 34];
            let ignore_portraits = [143, 144, 177, 415];

            for pokemon_id in start_id..=end_id {
                if ignore_portraits.contains(&pokemon_id) {
                    continue;
                }

                for &emotion_index in &emotion_indices {
                    if pokemon_id == 37 {
                        continue;
                    }
                    if pokemon_id == 64 && emotion_index > 4 {
                        continue;
                    }

                    if let Ok(Some(portrait)) =
                        kao_file_clone.get_portrait(pokemon_id, emotion_index)
                    {
                        let output_path = output_dir_clone.join(format!(
                            "pokemon_{:03}_emotion_{}.png",
                            pokemon_id + 1,
                            emotion_index
                        ));

                        match portrait.to_rgba_image() {
                            Ok(image) => {
                                if let Err(e) = image.save(&output_path) {
                                    println!(
                                        "Warning: Failed to save portrait {} emotion {}: {}",
                                        pokemon_id + 1,
                                        emotion_index,
                                        e
                                    );
                                } else {
                                    local_count += 1;
                                }
                            }
                            Err(e) => {
                                println!(
                        "Warning: Failed to convert portrait {} emotion {} to image: {}",
                        pokemon_id, emotion_index, e
                    );
                            }
                        }
                    }
                }
            }

            let mut global_count = count_clone.lock().unwrap();
            *global_count += local_count;
            println!("Thread {} extracted {} portraits", thread_id, local_count);
        });

        handles.push(handle);
    }

    for handle in handles {
        if let Err(e) = handle.join() {
            println!("Warning: Thread panicked: {:?}", e);
        }
    }

    let final_count = *extracted_count.lock().unwrap();
    println!("Successfully extracted {} portraits", final_count);
    Ok(())
}
