use std::collections::VecDeque;

// Constants for the PX algorithm
const PX_LOOKBACK_BUFFER_SIZE: usize = 4096;
const PX_MAX_MATCH_SEQLEN: usize = 18;
const PX_MIN_MATCH_SEQLEN: usize = 3;
const PX_NB_POSSIBLE_SEQUENCES_LEN: usize = 7;
const PX_MINIMUM_COMPRESSED_SIZE: usize = 9;

/// Enum for compression operation types
#[derive(Debug, Clone, Copy, PartialEq)]
enum Operation {
    CopyAsIs = -1,
    CopyNybble4Times = 0,
    CopyNybble4TimesExIncrallDecrnybble0 = 1,
    CopyNybble4TimesExDecrnybble1 = 2,
    CopyNybble4TimesExDecrnybble2 = 3,
    CopyNybble4TimesExDecrnybble3 = 4,
    CopyNybble4TimesExDecrallIncrnybble0 = 5,
    CopyNybble4TimesExIncrnybble1 = 6,
    CopyNybble4TimesExIncrnybble2 = 7,
    CopyNybble4TimesExIncrnybble3 = 8,
    CopySequence = 9,
}

/// Compression levels for PX compression
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PXCompLevel {
    /// No compression - All command bytes are 0xFF, and values are stored uncompressed
    Level0 = 0,
    /// Low compression - Handle 4 byte patterns using only control flag 0
    Level1 = 1,
    /// Medium compression - Handle 4 byte patterns using all control flags
    Level2 = 2,
    /// Full compression - Handle everything above, plus repeating sequences
    Level3 = 3,
}

/// Stores an operation to insert into the output buffer
#[derive(Debug, Clone)]
struct CompOp {
    op_type: Operation,
    high_nibble: u8,
    low_nibble: u8,
    next_byte_value: u8,
}

impl CompOp {
    fn new() -> Self {
        CompOp {
            op_type: Operation::CopyAsIs,
            high_nibble: 0,
            low_nibble: 0,
            next_byte_value: 0,
        }
    }
}

/// Represents a matching sequence for LZ77-style compression
#[derive(Debug, Clone)]
struct MatchingSeq {
    pos: usize,
    length: usize,
}

impl MatchingSeq {
    fn new(pos: usize, length: usize) -> Self {
        MatchingSeq { pos, length }
    }
}

/// PX compressor that handles compression using the PX algorithm
pub struct PxCompressor<'a> {
    uncompressed_data: &'a [u8],
    compression_level: PXCompLevel,
    should_search_first: bool,
    control_flags: Vec<u8>,
    compressed_data: Vec<u8>,
    pending_operations: VecDeque<CompOp>,
    high_nibble_lengths_possible: Vec<u8>,
    nb_compressed_byte_written: usize,
    cursor: usize,
    output_cursor: usize,
    input_size: usize,
}

impl<'a> PxCompressor<'a> {
    /// Create a new PX compressor
    pub fn new(
        uncompressed_data: &'a [u8],
        compression_level: PXCompLevel,
        should_search_first: bool,
    ) -> Self {
        let input_size = uncompressed_data.len();
        PxCompressor {
            uncompressed_data,
            compression_level,
            should_search_first,
            control_flags: Vec::new(),
            compressed_data: Vec::new(),
            pending_operations: VecDeque::new(),
            high_nibble_lengths_possible: Vec::new(),
            nb_compressed_byte_written: 0,
            cursor: 0,
            output_cursor: 0,
            input_size,
        }
    }

    /// Reset the compressor state
    fn reset(&mut self) {
        self.control_flags = Vec::new();
        self.compressed_data = Vec::new();
        self.pending_operations = VecDeque::new();
        self.high_nibble_lengths_possible = Vec::new();
        self.nb_compressed_byte_written = 0;
        self.cursor = 0;
        self.output_cursor = 0;
    }

    /// Compress the input data
    pub fn compress(&mut self) -> Result<(Vec<u8>, Vec<u8>), String> {
        self.reset();

        // Verify if we overflow
        if self.input_size > 2147483647 {
            return Err(format!(
                "PX Compression: The input data is too long {}. Max size: 2147483647 [max 32bit int]",
                self.input_size
            ));
        }

        // Allocate buffer for output - worst case scenario
        let extra = self.input_size / 8 + if self.input_size % 8 != 0 { 1 } else { 0 };
        let buffer_size = self.input_size + extra;
        self.compressed_data = vec![0; buffer_size];

        // Set default high nibble lengths
        self.high_nibble_lengths_possible.push(0);
        self.high_nibble_lengths_possible.push(0xF);

        // Process data in blocks
        while self.handle_a_block() {}

        // Build control flag table
        self.build_ctrl_flags_list();

        // Execute all operations from the queue
        self.output_all_operations();

        // Validate compressed size
        if self.nb_compressed_byte_written > 65536 {
            return Err(format!(
                "PX Compression: Compressed size {} overflows 16 bits unsigned integer!",
                self.nb_compressed_byte_written
            ));
        }

        // Truncate the output to the actual size
        self.compressed_data.truncate(self.output_cursor);

        Ok((self.control_flags.clone(), self.compressed_data.clone()))
    }

    /// Handle a block of up to 8 bytes
    fn handle_a_block(&mut self) -> bool {
        if self.cursor < self.input_size {
            // Determine what to do for as much bytes as possible
            for _ in 0..8 {
                if self.cursor >= self.input_size {
                    break;
                }

                // Determine the best operation first, then push it
                let operation = self.determine_best_operation();
                self.pending_operations.push_back(operation);
            }
            true
        } else {
            false
        }
    }

    /// Determine the best compression operation for current position
    fn determine_best_operation(&mut self) -> CompOp {
        let mut operation = CompOp::new();
        let mut amount_to_advance = 0;

        if self.should_search_first && self.compression_level as i32 >= PXCompLevel::Level3 as i32 {
            // Store the cursor position in a local variable
            let current_cursor = self.cursor;
            // Use a separate mutable variable to check if we can use a matching sequence
            let mut can_use_sequence = false;
            {
                can_use_sequence = self.can_use_a_matching_sequence(current_cursor, &mut operation);
            }

            if can_use_sequence {
                amount_to_advance = operation.high_nibble as usize + PX_MIN_MATCH_SEQLEN;
            }
        } else if self.compression_level as i32 >= PXCompLevel::Level1 as i32
            && self.can_compress_to_2_in_1_byte(self.cursor, &mut operation)
        {
            amount_to_advance = 2;
        } else if self.compression_level as i32 >= PXCompLevel::Level2 as i32
            && self.can_compress_to_2_in_1_byte_with_manipulation(self.cursor, &mut operation)
        {
            amount_to_advance = 2;
        } else if !self.should_search_first
            && self.compression_level as i32 >= PXCompLevel::Level3 as i32
        {
            // Store the cursor position in a local variable
            let current_cursor = self.cursor;
            // Use a separate mutable variable to check if we can use a matching sequence
            let mut can_use_sequence = false;
            {
                can_use_sequence = self.can_use_a_matching_sequence(current_cursor, &mut operation);
            }

            if can_use_sequence {
                amount_to_advance = operation.high_nibble as usize + PX_MIN_MATCH_SEQLEN;
            }
        } else {
            // If all else fails, add the byte as-is
            let b = self.uncompressed_data[self.cursor];
            operation.op_type = Operation::CopyAsIs;
            operation.high_nibble = (b >> 4) & 0x0F;
            operation.low_nibble = b & 0x0F;
            amount_to_advance = 1;
        }

        // Advance the cursor
        self.cursor += amount_to_advance;

        operation
    }

    /// Check if two bytes can be compressed as a single byte with repeated nibbles
    fn can_compress_to_2_in_1_byte(&self, cursor: usize, out_result: &mut CompOp) -> bool {
        if cursor + 1 >= self.input_size {
            return false;
        }

        // Get two bytes
        let both_bytes = ((self.uncompressed_data[cursor] as u16) << 8)
            | (self.uncompressed_data[cursor + 1] as u16);

        // Store low nibble
        out_result.low_nibble = (both_bytes & 0x0F) as u8;

        // Check if all nibbles match
        for i in 0..4 {
            let nibble = ((both_bytes >> (4 * i)) & 0x0F) as u8;
            if nibble != out_result.low_nibble {
                return false;
            }
        }

        out_result.op_type = Operation::CopyNybble4Times;
        true
    }

    /// Check if two bytes can be compressed using special operations
    fn can_compress_to_2_in_1_byte_with_manipulation(
        &self,
        cursor: usize,
        out_result: &mut CompOp,
    ) -> bool {
        if cursor + 1 >= self.input_size {
            return false;
        }

        // Get the two bytes
        let byte1 = self.uncompressed_data[cursor];
        let byte2 = self.uncompressed_data[cursor + 1];

        // Extract all 4 nibbles
        let nibbles = [
            (byte1 >> 4) & 0x0F,
            byte1 & 0x0F,
            (byte2 >> 4) & 0x0F,
            byte2 & 0x0F,
        ];

        // Count occurrences of each nibble
        let mut nibble_matches = [0u8; 4];
        for i in 0..4 {
            for j in 0..4 {
                if nibbles[i] == nibbles[j] {
                    nibble_matches[i] += 1;
                }
            }
        }

        // We need at least 3 values that occur 3 times
        let count_of_3 = nibble_matches.iter().filter(|&&count| count == 3).count();
        if count_of_3 < 3 {
            return false;
        }

        // Find min and max values
        let min_val = *nibbles.iter().min().unwrap();
        let max_val = *nibbles.iter().max().unwrap();

        // Check if difference is exactly 1
        if max_val - min_val != 1 {
            return false;
        }

        // Find index of min value
        let min_idx = nibbles.iter().position(|&x| x == min_val).unwrap();
        // Find index of max value
        let max_idx = nibbles.iter().position(|&x| x == max_val).unwrap();

        // Determine which case we're dealing with
        if nibble_matches[min_idx] == 1 {
            // This is the case where one nibble is smaller (Case A)
            out_result.op_type = match min_idx {
                0 => Operation::CopyNybble4TimesExIncrallDecrnybble0,
                1 => Operation::CopyNybble4TimesExDecrnybble1,
                2 => Operation::CopyNybble4TimesExDecrnybble2,
                3 => Operation::CopyNybble4TimesExDecrnybble3,
                _ => unreachable!(),
            };

            // Set the low nibble
            if min_idx == 0 {
                // For index 0, we keep as-is (it gets incremented then decremented)
                out_result.low_nibble = min_val;
            } else {
                // For others, add 1 (since we'll decrement during decompression)
                out_result.low_nibble = min_val + 1;
            }

            return true;
        } else if nibble_matches[max_idx] == 1 {
            // This is the case where one nibble is larger (Case B)
            out_result.op_type = match max_idx {
                0 => Operation::CopyNybble4TimesExDecrallIncrnybble0,
                1 => Operation::CopyNybble4TimesExIncrnybble1,
                2 => Operation::CopyNybble4TimesExIncrnybble2,
                3 => Operation::CopyNybble4TimesExIncrnybble3,
                _ => unreachable!(),
            };

            // Set the low nibble
            if max_idx == 0 {
                // For index 0, we keep as-is (it gets decremented then incremented)
                out_result.low_nibble = max_val;
            } else {
                // For others, subtract 1 (since we'll increment during decompression)
                out_result.low_nibble = max_val - 1;
            }

            return true;
        }

        false
    }

    /// Check if we can use a matching sequence for compression
    fn can_use_a_matching_sequence(&mut self, cursor: usize, out_result: &mut CompOp) -> bool {
        // Get offset of LookBack Buffer beginning
        let lb_buffer_begin = if cursor > PX_LOOKBACK_BUFFER_SIZE {
            cursor - PX_LOOKBACK_BUFFER_SIZE
        } else {
            0
        };

        // Setup iterators for clarity
        let it_look_back_begin = lb_buffer_begin;
        let it_look_back_end = cursor;
        let it_seq_begin = cursor;
        let it_seq_end = self.adv_as_much_as_possible(cursor, self.input_size, PX_MAX_MATCH_SEQLEN);

        let cur_seq_len = it_seq_end - it_seq_begin;

        // Make sure our sequence is at least three bytes long
        if cur_seq_len < PX_MIN_MATCH_SEQLEN {
            return false;
        }

        // Find the longest matching sequence
        let result = self.find_longest_matching_sequence(
            it_look_back_begin,
            it_look_back_end,
            it_seq_begin,
            it_seq_end,
            cur_seq_len,
        );

        if result.length >= PX_MIN_MATCH_SEQLEN {
            // Subtract 3 given that's how they're stored
            let valid_high_nibble = result.length - PX_MIN_MATCH_SEQLEN;

            // Check if the length is valid or can be added
            if !self.check_sequence_high_nibble_valid_or_add(valid_high_nibble as u8) {
                // If not valid and we can't add it, find the best length from our list
                let mut best_high_nibble = 0;
                for &len in &self.high_nibble_lengths_possible {
                    if (len as usize + PX_MIN_MATCH_SEQLEN) < result.length {
                        best_high_nibble = len;
                    }
                }

                out_result.high_nibble = best_high_nibble;
            } else {
                out_result.high_nibble = valid_high_nibble as u8;
            }

            // Calculate negative offset
            let signed_offset = -(cursor as i32 - result.pos as i32);

            // Set the output data
            out_result.low_nibble = ((signed_offset >> 8) & 0x0F) as u8;
            out_result.next_byte_value = (signed_offset & 0xFF) as u8;
            out_result.op_type = Operation::CopySequence;

            return true;
        }

        false
    }

    /// Find the longest matching sequence of at least PX_MIN_MATCH_SEQLEN bytes
    fn find_longest_matching_sequence(
        &self,
        search_beg: usize,
        search_end: usize,
        to_find_beg: usize,
        to_find_end: usize,
        _sequence_length: usize, // Added underscore to acknowledge it's intentionally unused
    ) -> MatchingSeq {
        let mut longest_match = MatchingSeq::new(search_end, 0);

        // Ensure we have enough to search
        if to_find_beg + PX_MIN_MATCH_SEQLEN > to_find_end {
            return longest_match;
        }

        // Get the minimum sequence we need to match
        let min_seq = &self.uncompressed_data[to_find_beg..to_find_beg + PX_MIN_MATCH_SEQLEN];

        let mut cur_search_pos = search_beg;
        while cur_search_pos < search_end {
            // Look for the next occurrence of the minimum sequence
            let mut found_pos = None;

            'outer: for i in cur_search_pos..search_end.saturating_sub(PX_MIN_MATCH_SEQLEN - 1) {
                let mut match_found = true;

                for j in 0..PX_MIN_MATCH_SEQLEN {
                    if self.uncompressed_data[i + j] != min_seq[j] {
                        match_found = false;
                        break;
                    }
                }

                if match_found {
                    found_pos = Some(i);
                    break 'outer;
                }
            }

            if let Some(pos) = found_pos {
                cur_search_pos = pos;

                // Count how many consecutive bytes match
                let nb_matches = self.count_equal_consecutive_elem(
                    cur_search_pos,
                    self.adv_as_much_as_possible(cur_search_pos, search_end, PX_MAX_MATCH_SEQLEN),
                    to_find_beg,
                    to_find_end,
                );

                // Update longest match if this one is better
                if longest_match.length < nb_matches {
                    longest_match.length = nb_matches;
                    longest_match.pos = cur_search_pos;
                }

                // If we found a match of maximum length, return immediately
                if nb_matches == PX_MAX_MATCH_SEQLEN {
                    return longest_match;
                }

                // Move to next position
                cur_search_pos += 1;
            } else {
                // No more matches found
                break;
            }
        }

        longest_match
    }

    /// Check if a high nibble value is valid for sequence length encoding
    fn check_sequence_high_nibble_valid_or_add(&mut self, hnibble_or_len: u8) -> bool {
        // Check if already in our list
        if self.high_nibble_lengths_possible.contains(&hnibble_or_len) {
            return true;
        }

        // If not in list, check if we can add it
        if self.high_nibble_lengths_possible.len() < PX_NB_POSSIBLE_SEQUENCES_LEN {
            self.high_nibble_lengths_possible.push(hnibble_or_len);
            self.high_nibble_lengths_possible.sort();
            return true;
        }

        // Can't add it - already at max capacity
        false
    }

    /// Output a single operation into the compressed data
    fn output_an_operation(&mut self, operation: &CompOp) {
        let insert_pos = self.output_cursor;

        match operation.op_type {
            Operation::CopyAsIs => {
                self.compressed_data[insert_pos] =
                    (operation.high_nibble << 4) | operation.low_nibble;
                self.output_cursor += 1;
                self.nb_compressed_byte_written += 1;
            }
            Operation::CopySequence => {
                self.compressed_data[insert_pos] =
                    (operation.high_nibble << 4) | operation.low_nibble;
                self.output_cursor += 1;
                self.nb_compressed_byte_written += 1;

                self.compressed_data[insert_pos + 1] = operation.next_byte_value;
                self.output_cursor += 1;
                self.nb_compressed_byte_written += 1;
            }
            _ => {
                // For pattern operations, use the corresponding control flag
                let flag = self.control_flags[operation.op_type as usize];
                self.compressed_data[insert_pos] = (flag << 4) | operation.low_nibble;
                self.output_cursor += 1;
                self.nb_compressed_byte_written += 1;
            }
        }
    }

    /// Build the control flags list based on sequence lengths
    fn build_ctrl_flags_list(&mut self) {
        // Make sure we have PX_NB_POSSIBLE_SEQUENCES_LEN values for lengths
        while self.high_nibble_lengths_possible.len() < PX_NB_POSSIBLE_SEQUENCES_LEN {
            for nibble_val in 0..0xF {
                if !self.high_nibble_lengths_possible.contains(&nibble_val) {
                    self.high_nibble_lengths_possible.push(nibble_val);
                    if self.high_nibble_lengths_possible.len() == PX_NB_POSSIBLE_SEQUENCES_LEN {
                        break;
                    }
                }
            }
        }

        // Create control flags array
        self.control_flags = vec![0; 9];
        let mut ctrl_flag_insert = 0;

        // Assign flag values that aren't used for sequence lengths
        for flag_val in 0..0xF {
            if !self.high_nibble_lengths_possible.contains(&flag_val)
                && ctrl_flag_insert < self.control_flags.len()
            {
                self.control_flags[ctrl_flag_insert] = flag_val;
                ctrl_flag_insert += 1;
            }
        }
    }

    /// Output all operations from the queue
    fn output_all_operations(&mut self) {
        // Process operations in blocks of 8
        while !self.pending_operations.is_empty() {
            // Create command byte using the first 8 operations
            let mut command_byte = 0;

            for i in 0..8 {
                if i >= self.pending_operations.len() {
                    break;
                }

                // Set bit to 1 only for CopyAsIs operations
                if self.pending_operations[i].op_type == Operation::CopyAsIs {
                    command_byte |= 1 << (7 - i);
                }
            }

            // Output command byte
            self.compressed_data[self.output_cursor] = command_byte;
            self.output_cursor += 1;
            self.nb_compressed_byte_written += 1;

            // Process up to 8 operations
            for _ in 0..8 {
                if self.pending_operations.is_empty() {
                    break;
                }

                if let Some(op) = self.pending_operations.pop_front() {
                    self.output_an_operation(&op);
                }
            }
        }
    }

    /// Advance an iterator as much as possible without exceeding limit
    fn adv_as_much_as_possible(&self, iter: usize, iter_end: usize, displacement: usize) -> usize {
        if iter + displacement > iter_end {
            iter_end
        } else {
            iter + displacement
        }
    }

    /// Count equal consecutive elements between two sequences
    fn count_equal_consecutive_elem(
        &self,
        first_1: usize,
        last_1: usize,
        first_2: usize,
        last_2: usize,
    ) -> usize {
        let mut count = 0;
        let mut pos_1 = first_1;
        let mut pos_2 = first_2;

        while pos_1 < last_1
            && pos_2 < last_2
            && self.uncompressed_data[pos_1] == self.uncompressed_data[pos_2]
        {
            count += 1;
            pos_1 += 1;
            pos_2 += 1;
        }

        count
    }
}

/// PX decompressor handles decompression of PX compressed data
pub struct PxDecompressor<'a> {
    compressed_data: &'a [u8],
    flags: &'a [u8],
    cursor: usize,
    uncompressed_data: Vec<u8>,
}

impl<'a> PxDecompressor<'a> {
    /// Create a new PX decompressor
    pub fn new(compressed_data: &'a [u8], flags: &'a [u8]) -> Self {
        PxDecompressor {
            compressed_data,
            flags,
            cursor: 0,
            uncompressed_data: Vec::new(),
        }
    }

    /// Reset the decompressor state
    fn reset(&mut self) {
        self.cursor = 0;
        self.uncompressed_data = Vec::new();
    }

    /// Decompress the data
    pub fn decompress(&mut self) -> Result<Vec<u8>, String> {
        self.reset();

        let c_data_len = self.compressed_data.len();
        while self.cursor < c_data_len {
            self.handle_control_byte(c_data_len)?;
        }

        Ok(self.uncompressed_data.clone())
    }

    /// Handle a control byte and its associated operations
    fn handle_control_byte(&mut self, c_data_len: usize) -> Result<(), String> {
        if self.cursor >= c_data_len {
            return Ok(());
        }

        // Read the control byte first and store it
        let ctrl_byte = self.read_next_byte();

        // Process each bit in the control byte
        for bit_pos in 0..8 {
            if self.cursor >= c_data_len {
                break;
            }

            let ctrl_bit = (ctrl_byte & (1 << (7 - bit_pos))) != 0;

            if ctrl_bit {
                // Direct copy of the next byte - read first then push
                let next_byte = self.read_next_byte();
                self.uncompressed_data.push(next_byte);
            } else {
                // Handle special case
                self.handle_special_case()?;
            }
        }

        Ok(())
    }

    /// Handle a special compression case
    fn handle_special_case(&mut self) -> Result<(), String> {
        let next_byte = self.read_next_byte();
        let high_nibble = (next_byte >> 4) & 0x0F;
        let low_nibble = next_byte & 0x0F;

        // Check if high nibble matches any control flag
        let idx_ctrl_flags = self.matches_flags(high_nibble);

        if let Some(idx) = idx_ctrl_flags {
            // Handle pattern-based compression
            let pattern = compute_four_nibbles_pattern(idx, low_nibble);
            self.uncompressed_data.extend_from_slice(&pattern);
        } else {
            // Store current values to avoid multiple mutable borrows
            let current_low_nibble = low_nibble;
            let current_high_nibble = high_nibble;

            // Use a separate method call to copy the sequence
            self.copy_sequence(current_low_nibble, current_high_nibble)?;
        }

        Ok(())
    }

    /// Read next byte and advance cursor
    fn read_next_byte(&mut self) -> u8 {
        let b = self.compressed_data[self.cursor];
        self.cursor += 1;
        b
    }

    /// Check if a nibble matches any control flag
    fn matches_flags(&self, high_nibble: u8) -> Option<usize> {
        for (idx, &flag) in self.flags.iter().enumerate() {
            if flag == high_nibble {
                return Some(idx);
            }
        }
        None
    }

    /// Insert a 2-byte pattern based on control flag and low nibble
    fn insert_byte_pattern(&mut self, idx_ctrl_flags: usize, low_nibble: u8) {
        // Based on the control flag, build two new bytes
        let two_bytes = compute_four_nibbles_pattern(idx_ctrl_flags, low_nibble);
        self.uncompressed_data.extend_from_slice(&two_bytes);
    }

    /// Copy a sequence from previously decompressed data
    fn copy_sequence(&mut self, low_nibble: u8, high_nibble: u8) -> Result<(), String> {
        // Read offset byte
        let offset_byte = self.read_next_byte();

        // Calculate offset
        let offset = (-0x1000 + ((low_nibble as i32) << 8) | (offset_byte as i32)) as isize;

        // Get current position
        let out_cur_byte = self.uncompressed_data.len();

        // Check if offset is valid
        if offset.abs() as usize > out_cur_byte {
            return Err(format!(
                "Sequence to copy out of bound! Expected max. {} but got {}",
                out_cur_byte, offset
            ));
        }

        // Calculate sequence length
        let bytes_to_copy = (high_nibble as usize) + PX_MIN_MATCH_SEQLEN;

        // Calculate position to copy from
        let copy_pos = (out_cur_byte as isize + offset) as usize;

        // Copy data sequence
        for i in 0..bytes_to_copy {
            let byte = self.uncompressed_data[copy_pos + i];
            self.uncompressed_data.push(byte);
        }

        Ok(())
    }
}

/// Compute a pattern of 4 nibbles based on control flag and low nibble
fn compute_four_nibbles_pattern(idx_ctrl_flags: usize, low_nibble: u8) -> [u8; 2] {
    if idx_ctrl_flags == 0 {
        // In this case, all 4 nibbles have the same value
        let byte_val = (low_nibble << 4) | low_nibble;
        [byte_val, byte_val]
    } else {
        // Here we handle 2 special cases together
        let mut nibble_base = low_nibble;

        // At these indices exactly, the base value for all nibbles has to be changed:
        if idx_ctrl_flags == 1 {
            nibble_base = nibble_base.wrapping_add(1);
        } else if idx_ctrl_flags == 5 {
            nibble_base = nibble_base.wrapping_sub(1);
        }

        // Create array with all nibbles set to the base value
        let mut ns = [nibble_base; 4];

        // In these cases, only specific nibbles have to be changed:
        if (1..=4).contains(&idx_ctrl_flags) {
            ns[idx_ctrl_flags - 1] = ns[idx_ctrl_flags - 1].wrapping_sub(1);
        } else if (5..=8).contains(&idx_ctrl_flags) {
            ns[idx_ctrl_flags - 5] = ns[idx_ctrl_flags - 5].wrapping_add(1);
        }

        // Combine nibbles into bytes
        [(ns[0] << 4) | ns[1], (ns[2] << 4) | ns[3]]
    }
}

/// Handler for PX compression and decompression
pub struct PxHandler;

impl PxHandler {
    /// Decompress data stored in PX format
    pub fn decompress(compressed_data: &[u8], flags: &[u8]) -> Result<Vec<u8>, String> {
        let mut decompressor = PxDecompressor::new(compressed_data, flags);
        decompressor.decompress()
    }

    /// Compress data using the PX algorithm
    pub fn compress(uncompressed_data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
        // Use Level3 compression by default
        let mut compressor = PxCompressor::new(uncompressed_data, PXCompLevel::Level3, true);
        compressor.compress()
    }
}

/// Iterate over the bits of a byte
fn iter_bits(byte: u8) -> impl Iterator<Item = u8> {
    (0..8).map(move |bit_pos| (byte >> (7 - bit_pos)) & 1)
}
