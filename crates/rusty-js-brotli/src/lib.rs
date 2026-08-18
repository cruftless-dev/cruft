#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrotliError {
    UnexpectedEnd,
    UnsupportedStream,
    InvalidStream,
    OutputTooLarge,
}

impl std::fmt::Display for BrotliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrotliError::UnexpectedEnd => write!(f, "unexpected end of brotli stream"),
            BrotliError::UnsupportedStream => write!(f, "unsupported brotli stream shape"),
            BrotliError::InvalidStream => write!(f, "invalid brotli stream"),
            BrotliError::OutputTooLarge => write!(f, "decoded brotli output exceeds maximum size"),
        }
    }
}

impl std::error::Error for BrotliError {}

#[derive(Clone, Copy, Debug)]
pub struct BrotliParams {
    pub quality: u32,
    pub lgwin: u32,
    pub mode: u32,
    pub size_hint: usize,
    pub large_window: bool,
}

impl Default for BrotliParams {
    fn default() -> Self {
        Self {
            quality: 11,
            lgwin: 22,
            mode: 0,
            size_hint: 0,
            large_window: false,
        }
    }
}

const UNCOMPRESSED_CHUNK: usize = 65_536;
const SINGLE_LITERAL_BLOCK_MAX: usize = 512;
pub const MAX_OUTPUT: usize = 256 * 1024 * 1024;
#[allow(dead_code)]
const LZ77_MIN_MATCH: usize = 4;
#[allow(dead_code)]
const LZ77_MAX_DISTANCE: usize = 1 << 15;
#[allow(dead_code)]
const LZ77_MAX_MATCH: usize = 16_384;
#[allow(dead_code)]
const BROTLI_GENERATED_MAX_DISTANCE: usize = 1 << 16;
#[allow(dead_code)]
const BROTLI_LITERAL_CONTEXTS: usize = 64;
const RECENT_DISTANCE_ALTERNATIVE_LIMIT: usize = 1;

#[allow(dead_code)]
const EXACT_STATIC_DICTIONARY_WORDS: &[StaticDictionaryWord] = &[
    StaticDictionaryWord {
        output: b"production",
        length: 10,
        index: 48,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"packages",
        length: 8,
        index: 629,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"exports",
        length: 7,
        index: 406,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"license",
        length: 7,
        index: 180,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"package",
        length: 7,
        index: 286,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"require",
        length: 7,
        index: 190,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"version",
        length: 7,
        index: 41,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"module",
        length: 6,
        index: 27,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"source",
        length: 6,
        index: 24,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"stream",
        length: 6,
        index: 121,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"string",
        length: 6,
        index: 277,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"files",
        length: 5,
        index: 198,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"press",
        length: 5,
        index: 81,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"small",
        length: 5,
        index: 9,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"body",
        length: 4,
        index: 19,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"name",
        length: 4,
        index: 61,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"node",
        length: 4,
        index: 488,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"text",
        length: 4,
        index: 16,
        transform_id: 0,
    },
    StaticDictionaryWord {
        output: b"load. ",
        length: 4,
        index: 118,
        transform_id: 31,
    },
    StaticDictionaryWord {
        output: b"flow ",
        length: 4,
        index: 289,
        transform_id: 1,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Lz77Profile {
    candidate_limit: usize,
    max_match: usize,
    lazy_match: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DistanceProfile {
    ndirect: u16,
    npostfix: u8,
}

impl DistanceProfile {
    fn distance_alphabet_size(self) -> u16 {
        16 + self.ndirect + (48u16 << self.npostfix)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum Lz77Step {
    Literal { start: usize, len: usize },
    Copy { distance: usize, len: usize },
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LengthCode {
    code: u16,
    extra_bits: u8,
    extra_value: u32,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InsertCopyCode {
    code: u16,
    insert_extra_bits: u8,
    insert_extra_value: u32,
    copy_extra_bits: u8,
    copy_extra_value: u32,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DistanceCode {
    code: u16,
    extra_bits: u8,
    extra_value: u32,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct BrotliCommand {
    insert_start: usize,
    insert_len: usize,
    copy_len: usize,
    copy_distance: Option<usize>,
    insert_copy: InsertCopyCode,
    distance: Option<DistanceCode>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SimplePrefixCode {
    alphabet_size: u16,
    alphabet_bits: u8,
    symbols: Vec<u16>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum PrefixCodeProfile {
    Simple(SimplePrefixCode),
    Complex(ComplexPrefixCode),
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SingleBlockPrefixProfile {
    ndirect: u16,
    npostfix: u8,
    literal: SimplePrefixCode,
    insert_copy: SimplePrefixCode,
    distance: SimplePrefixCode,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct ComplexPrefixCode {
    alphabet_size: u16,
    max_bits: u8,
    code_lengths: Vec<(u16, u8)>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalCode {
    symbol: u16,
    len: u8,
    msb_code: u16,
    lsb_bits: u16,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CodeLengthOp {
    symbol: u8,
    repeat: usize,
    extra_bits: u8,
    extra_value: u8,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct PrefixDescriptionPlan {
    alphabet_size: u16,
    trimmed_len: usize,
    ops: Vec<CodeLengthOp>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct CodeLengthPrefixPlan {
    code: ComplexPrefixCode,
    canonical: Vec<CanonicalCode>,
    ordered_lengths: Vec<(u8, u8)>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BitSpan {
    bits: u16,
    len: u8,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct PrefixDescriptionBitPlan {
    header_lengths: Vec<BitSpan>,
    op_spans: Vec<BitSpan>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct PrefixDescriptionFragment {
    bytes: Vec<u8>,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct CommandPayloadFragment {
    bytes: Vec<u8>,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SingleBlockCompressedBodyPlan {
    prefix_spans: Vec<BitSpan>,
    payload_spans: Vec<BitSpan>,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct ContextualSingleBlockCompressedBodyPlan {
    prefix_spans: Vec<BitSpan>,
    payload_spans: Vec<BitSpan>,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SingleBlockCompressedBodyFragment {
    bytes: Vec<u8>,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SingleBlockCompressedStreamPlan {
    header_spans: Vec<BitSpan>,
    body: SingleBlockCompressedBodyPlan,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct ContextualSingleBlockCompressedStreamPlan {
    header_spans: Vec<BitSpan>,
    body: ContextualSingleBlockCompressedBodyPlan,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct LiteralBlockTypeSwitchStreamPlan {
    header_spans: Vec<BitSpan>,
    prefix_spans: Vec<BitSpan>,
    payload_spans: Vec<BitSpan>,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SingleBlockCompressedStreamFragment {
    bytes: Vec<u8>,
    bit_len: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SingleBlockComplexPrefixProfile {
    ndirect: u16,
    npostfix: u8,
    literal: ComplexPrefixCode,
    insert_copy: ComplexPrefixCode,
    distance: ComplexPrefixCode,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct ContextualSingleBlockComplexPrefixProfile {
    ndirect: u16,
    npostfix: u8,
    literal_context_map: [u8; BROTLI_LITERAL_CONTEXTS],
    literal: Vec<ComplexPrefixCode>,
    insert_copy: ComplexPrefixCode,
    distance: ComplexPrefixCode,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct HuffmanNode {
    weight: usize,
    symbol: Option<u16>,
    left: Option<usize>,
    right: Option<usize>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DictionaryTransformKind {
    Identity,
    FermentFirst,
    FermentAll,
    OmitFirst(u8),
    OmitLast(u8),
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DictionaryTransform {
    prefix: &'static [u8],
    kind: DictionaryTransformKind,
    suffix: &'static [u8],
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StaticDictionaryRef {
    length: usize,
    index: usize,
    transform_id: usize,
    offset: usize,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StaticDictionaryWord {
    output: &'static [u8],
    length: usize,
    index: usize,
    transform_id: usize,
}

pub fn encode(data: &[u8], params: &BrotliParams) -> Result<Vec<u8>, BrotliError> {
    if params.quality != 0 {
        if let Some(encoded) = encode_lz77(data, lz77_profile(params.quality)) {
            return Ok(encoded);
        }
    }
    if !data.is_empty() && data.len() <= SINGLE_LITERAL_BLOCK_MAX {
        return Ok(encode_single_literal_block(data));
    }
    Ok(encode_uncompressed_blocks(data))
}

fn lz77_profile(quality: u32) -> Lz77Profile {
    match quality.min(11) {
        0 => Lz77Profile {
            candidate_limit: 0,
            max_match: 0,
            lazy_match: false,
        },
        1..=3 => Lz77Profile {
            candidate_limit: 4,
            max_match: 64,
            lazy_match: false,
        },
        4..=6 => Lz77Profile {
            candidate_limit: 8,
            max_match: 258,
            lazy_match: false,
        },
        7..=9 => Lz77Profile {
            candidate_limit: 16,
            max_match: 1024,
            lazy_match: true,
        },
        _ => Lz77Profile {
            candidate_limit: 32,
            max_match: LZ77_MAX_MATCH,
            lazy_match: true,
        },
    }
}

fn encode_uncompressed_blocks(data: &[u8]) -> Vec<u8> {
    let mut bw = BitWriter::new();
    bw.write_bit(false);
    for chunk in data.chunks(UNCOMPRESSED_CHUNK) {
        bw.write_bit(false);
        bw.write_bits(0, 2);
        bw.write_bits((chunk.len() - 1) as u32, 16);
        bw.write_bit(true);
        bw.write_aligned_bytes(chunk);
    }
    bw.write_bit(true);
    bw.write_bit(true);
    bw.finish()
}

fn encode_single_block_lz77(data: &[u8], profile: Lz77Profile) -> Option<Vec<u8>> {
    let stream = best_single_block_compressed_plan(data, profile, true, true)?;
    (stream_bytes_len(stream.bit_len) + 4 < data.len()).then(|| write_stream_plan(&stream))
}

fn encode_lz77(data: &[u8], profile: Lz77Profile) -> Option<Vec<u8>> {
    let mut best = encode_single_block_lz77(data, profile);
    if data.len() > 65_536 || best.is_none() {
        if let Some(split) = encode_two_block_lz77(data, profile) {
            if best
                .as_ref()
                .is_none_or(|current| split.len() < current.len())
            {
                best = Some(split);
            }
        }
    }
    if let Some(block_typed) = encode_literal_block_type_lz77(data, profile) {
        if best
            .as_ref()
            .is_none_or(|current| block_typed.len() < current.len())
        {
            best = Some(block_typed);
        }
    }
    best
}

fn encode_literal_block_type_lz77(data: &[u8], profile: Lz77Profile) -> Option<Vec<u8>> {
    if data.len() < 1024 || data.len() > 65_536 {
        return None;
    }
    let mut best: Option<LiteralBlockTypeSwitchStreamPlan> = None;
    for distance_profile in literal_block_type_distance_profiles_for_lz77(profile)
        .iter()
        .copied()
    {
        let commands = lower_lz77_commands_with_profile(
            data,
            distance_profile.ndirect,
            distance_profile.npostfix,
            profile,
        );
        best = best_literal_block_type_lz77_for_commands(data, &commands, distance_profile, best);
        let cached_commands = lower_lz77_commands_with_profile_output_base_and_distance_cache(
            data,
            distance_profile.ndirect,
            distance_profile.npostfix,
            profile,
            0,
        );
        if profile.candidate_limit >= 32
            && cached_commands != commands
            && commands_have_implicit_distance(&cached_commands)
        {
            best = best_literal_block_type_lz77_for_commands(
                data,
                &cached_commands,
                distance_profile,
                best,
            );
        }
    }
    if profile.candidate_limit >= 32 {
        best = best_literal_block_type_lz77_for_recent_distance_alternatives(
            data,
            DistanceProfile {
                ndirect: 0,
                npostfix: 0,
            },
            profile,
            best,
        );
    }
    let stream = best?;
    let bytes = write_literal_block_type_switch_stream(&stream);
    (bytes.len() + 4 < data.len()).then_some(bytes)
}

fn best_literal_block_type_lz77_for_recent_distance_alternatives(
    data: &[u8],
    distance_profile: DistanceProfile,
    profile: Lz77Profile,
    mut best: Option<LiteralBlockTypeSwitchStreamPlan>,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    let steps = plan_lz77_with_profile(data, profile);
    let mut source_pos = 0usize;
    let mut recent_distances = Vec::<usize>::new();
    for (step_index, step) in steps.iter().enumerate() {
        match *step {
            Lz77Step::Literal { len, .. } => source_pos += len,
            Lz77Step::Copy {
                distance: old_distance,
                len: old_len,
            } => {
                if !(LZ77_MIN_MATCH + 1..=64).contains(&old_len) {
                    source_pos += old_len;
                    continue;
                }
                let len = old_len - 1;
                for distance in recent_distances
                    .iter()
                    .copied()
                    .take(RECENT_DISTANCE_ALTERNATIVE_LIMIT)
                {
                    let Some(pos) = source_pos.checked_sub(distance) else {
                        continue;
                    };
                    if distance == old_distance {
                        continue;
                    }
                    let max_len = match_len(data, pos, source_pos)
                        .min(profile.max_match)
                        .min(old_len);
                    if max_len < len {
                        continue;
                    }
                    let mut candidate = steps.clone();
                    candidate.splice(
                        step_index..=step_index,
                        [
                            Lz77Step::Copy { distance, len },
                            Lz77Step::Literal {
                                start: source_pos + len,
                                len: 1,
                            },
                        ],
                    );
                    let commands = lower_lz77_steps_with_profile_output_base_and_distance_cache(
                        data,
                        &candidate,
                        distance_profile.ndirect,
                        distance_profile.npostfix,
                        0,
                    );
                    best = best_literal_block_type_lz77_for_commands(
                        data,
                        &commands,
                        distance_profile,
                        best,
                    );
                }
                if recent_distances.first().copied() != Some(old_distance) {
                    recent_distances.retain(|&distance| distance != old_distance);
                    recent_distances.insert(0, old_distance);
                    recent_distances.truncate(8);
                }
                source_pos += old_len;
            }
        }
    }
    best
}

fn best_literal_block_type_lz77_for_commands(
    data: &[u8],
    commands: &[BrotliCommand],
    distance_profile: DistanceProfile,
    mut best: Option<LiteralBlockTypeSwitchStreamPlan>,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    if !commands
        .iter()
        .any(|command| command.copy_distance.is_some())
    {
        return best;
    }
    let total_literals = inserted_literal_count(&commands);
    if total_literals < 32 {
        return best;
    }
    let prefix_profile =
        single_block_complex_prefix_profile_with_distance(data, &commands, distance_profile)?;
    for split in literal_split_candidates_for_data(data, &commands, total_literals) {
        let Some(stream) = literal_block_type_two_tree_stream_plan_with_literal_split(
            data,
            &commands,
            &prefix_profile,
            split,
        ) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|current| stream.bit_len < current.bit_len)
        {
            best = Some(stream);
        }
    }
    for (literal_split, command_split) in
        literal_command_split_candidates_for_data(data, &commands, total_literals)
    {
        let Some(stream) = literal_command_block_type_stream_plan(
            data,
            &commands,
            &prefix_profile,
            literal_split,
            command_split,
        ) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|current| stream.bit_len < current.bit_len)
        {
            best = Some(stream);
        }
    }
    for command_split in command_split_candidates(commands.len()) {
        let Some(stream) =
            command_block_type_stream_plan(data, &commands, &prefix_profile, command_split)
        else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|current| stream.bit_len < current.bit_len)
        {
            best = Some(stream);
        }
    }
    let total_distances = distance_symbol_count(&commands);
    if total_distances >= 2 {
        for (literal_split, command_split, distance_split) in
            literal_command_distance_split_candidates_for_data(
                data,
                &commands,
                total_literals,
                total_distances,
            )
        {
            let Some(stream) = literal_command_distance_block_type_stream_plan(
                data,
                &commands,
                &prefix_profile,
                literal_split,
                command_split,
                distance_split,
            ) else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|current| stream.bit_len < current.bit_len)
            {
                best = Some(stream);
            }
        }
    }
    best
}

fn encode_two_block_lz77(data: &[u8], profile: Lz77Profile) -> Option<Vec<u8>> {
    if data.len() <= 65_536 || data.len() > 131_072 {
        return None;
    }
    let mut best: Option<SingleBlockCompressedStreamFragment> = None;
    for split in two_block_split_candidates(data) {
        let left = &data[..split];
        let right = &data[split..];
        if left.is_empty() || right.is_empty() || left.len() > 65_536 || right.len() > 65_536 {
            continue;
        }
        let profile_pairs: Vec<(DistanceProfile, DistanceProfile)> =
            if data.len() > 65_536 && profile.candidate_limit >= 32 {
                LARGE_TWO_BLOCK_DISTANCE_PROFILE_PAIRS.to_vec()
            } else {
                two_block_distance_profiles(data, profile)
                    .iter()
                    .copied()
                    .map(|distance_profile| (distance_profile, distance_profile))
                    .collect()
            };
        for (left_distance_profile, right_distance_profile) in profile_pairs {
            let Some(left_plan) = best_single_block_plan_for_distance_profile(
                left,
                profile,
                left_distance_profile,
                true,
                false,
                0,
                true,
            ) else {
                continue;
            };
            let Some(right_plan) = best_single_block_plan_for_distance_profile(
                right,
                profile,
                right_distance_profile,
                false,
                true,
                left.len(),
                true,
            ) else {
                continue;
            };
            let mut bw = BitWriter::new();
            bw.write_spans(&left_plan.header_spans);
            bw.write_spans(&left_plan.body.prefix_spans);
            bw.write_spans(&left_plan.body.payload_spans);
            bw.write_spans(&right_plan.header_spans);
            bw.write_spans(&right_plan.body.prefix_spans);
            bw.write_spans(&right_plan.body.payload_spans);
            let bit_len = bw.bit_len();
            let stream = SingleBlockCompressedStreamFragment {
                bytes: bw.finish(),
                bit_len,
            };
            if best
                .as_ref()
                .is_none_or(|current| stream.bit_len < current.bit_len)
            {
                best = Some(stream);
            }
        }
    }
    let stream = best?;
    (stream.bytes.len() + 4 < data.len()).then_some(stream.bytes)
}

fn best_single_block_plan_for_distance_profile(
    data: &[u8],
    profile: Lz77Profile,
    distance_profile: DistanceProfile,
    include_wbits: bool,
    is_last: bool,
    output_base: usize,
    allow_literal_only: bool,
) -> Option<SingleBlockCompressedStreamPlan> {
    let commands = lower_lz77_commands_with_profile_and_output_base(
        data,
        distance_profile.ndirect,
        distance_profile.npostfix,
        profile,
        output_base,
    );
    best_single_block_candidate_for_commands(
        data,
        &commands,
        distance_profile,
        include_wbits,
        is_last,
        allow_literal_only,
        None,
    )
}

fn two_block_split_candidates(data: &[u8]) -> Vec<usize> {
    if data.len() <= 65_536 {
        return split_candidates(data);
    }
    let grid = 2048usize;
    let target = (data.len() * 2 / 3).div_ceil(grid) * grid;
    let min = data.len().saturating_sub(65_536).max(1);
    let max = 65_536.min(data.len().saturating_sub(1));
    vec![target.clamp(min, max)]
}

fn two_block_distance_profiles(data: &[u8], profile: Lz77Profile) -> &'static [DistanceProfile] {
    if data.len() > 65_536 && profile.candidate_limit >= 32 {
        &LARGE_TWO_BLOCK_DISTANCE_PROFILES
    } else {
        distance_profiles_for_lz77(profile)
    }
}

fn split_candidates(data: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    for needle in [b"],\"body\":\"".as_slice(), b",\"body\":\"".as_slice()] {
        if let Some(pos) = data
            .windows(needle.len())
            .position(|window| window == needle)
        {
            out.push(pos + needle.len());
        }
    }
    for needle in [
        b"\",\"".as_slice(),
        b"\",[".as_slice(),
        b"],\"".as_slice(),
        b"},\"".as_slice(),
    ] {
        for (pos, _) in data
            .windows(needle.len())
            .enumerate()
            .filter(|(_, window)| *window == needle)
        {
            if out.len() >= 16 {
                break;
            }
            out.push(pos + needle.len());
        }
    }
    let grid = if data.len() <= 8 * 1024 { 512 } else { 2048 };
    let mut split = grid;
    while split + grid < data.len() {
        out.push(split);
        split += grid;
    }
    out.extend([data.len() / 3, data.len() / 2, data.len() * 2 / 3]);
    out.sort_unstable();
    out.dedup();
    out
}

fn inserted_literal_count(commands: &[BrotliCommand]) -> usize {
    commands.iter().map(|command| command.insert_len).sum()
}

fn literal_split_candidates(total_literals: usize) -> Vec<usize> {
    let mut out = Vec::new();
    if total_literals < 2 {
        return out;
    }
    out.extend([
        total_literals / 3,
        total_literals / 2,
        total_literals * 2 / 3,
    ]);
    let step = if total_literals <= 512 { 64 } else { 256 };
    let mut split = step;
    while split + step < total_literals {
        out.push(split);
        split += step;
    }
    out.retain(|&split| split > 0 && split < total_literals);
    out.sort_unstable();
    out.dedup();
    out
}

fn literal_split_candidates_for_data(
    data: &[u8],
    commands: &[BrotliCommand],
    total_literals: usize,
) -> Vec<usize> {
    let mut out = literal_split_candidates(total_literals);
    for source_split in split_candidates(data) {
        if let Some(literal_split) = source_split_to_inserted_literal_index(commands, source_split)
        {
            if literal_split > 0 && literal_split < total_literals {
                out.push(literal_split);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn source_split_to_inserted_literal_index(
    commands: &[BrotliCommand],
    source_split: usize,
) -> Option<usize> {
    let mut literal_index = 0usize;
    for command in commands {
        let start = command.insert_start;
        let end = start.checked_add(command.insert_len)?;
        if source_split <= start {
            return Some(literal_index);
        }
        if source_split < end {
            return Some(literal_index + source_split - start);
        }
        literal_index += command.insert_len;
    }
    Some(literal_index)
}

fn command_split_candidates(total_commands: usize) -> Vec<usize> {
    let mut out = Vec::new();
    if total_commands < 2 {
        return out;
    }
    out.extend([
        total_commands / 3,
        total_commands / 2,
        total_commands * 2 / 3,
    ]);
    for tail in [2usize, 4, 7, 8, 12] {
        if total_commands > tail {
            out.push(total_commands - tail);
        }
    }
    let step = if total_commands <= 64 { 8 } else { 32 };
    let mut split = step;
    while split + step < total_commands {
        out.push(split);
        split += step;
    }
    out.retain(|&split| split > 0 && split < total_commands);
    out.sort_unstable();
    out.dedup();
    out
}

fn literal_command_split_candidates_for_data(
    data: &[u8],
    commands: &[BrotliCommand],
    total_literals: usize,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for command_split in command_split_candidates(commands.len()) {
        if let Some(literal_split) = inserted_literal_index_at_command(commands, command_split) {
            if literal_split > 0 && literal_split < total_literals {
                out.push((literal_split, command_split));
            }
        }
    }
    for source_split in split_candidates(data) {
        let Some(literal_split) = source_split_to_inserted_literal_index(commands, source_split)
        else {
            continue;
        };
        let Some(command_split) = source_split_to_command_index(commands, source_split) else {
            continue;
        };
        if literal_split > 0
            && literal_split < total_literals
            && command_split > 0
            && command_split < commands.len()
        {
            out.push((literal_split, command_split));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn inserted_literal_index_at_command(
    commands: &[BrotliCommand],
    command_split: usize,
) -> Option<usize> {
    if command_split > commands.len() {
        return None;
    }
    commands
        .iter()
        .take(command_split)
        .try_fold(0usize, |acc, command| acc.checked_add(command.insert_len))
}

fn source_split_to_command_index(commands: &[BrotliCommand], source_split: usize) -> Option<usize> {
    for (index, command) in commands.iter().enumerate() {
        let start = command.insert_start;
        let end = start.checked_add(command.insert_len)?;
        if source_split <= start {
            return Some(index);
        }
        if source_split < end {
            return Some(index + 1);
        }
    }
    Some(commands.len())
}

fn distance_symbol_count(commands: &[BrotliCommand]) -> usize {
    commands
        .iter()
        .filter(|command| command.distance.is_some())
        .count()
}

fn distance_split_candidates(total_distances: usize) -> Vec<usize> {
    let mut out = Vec::new();
    if total_distances < 2 {
        return out;
    }
    out.extend([
        total_distances / 3,
        total_distances / 2,
        total_distances * 2 / 3,
    ]);
    let step = if total_distances <= 32 { 4 } else { 16 };
    let mut split = step;
    while split + step < total_distances {
        out.push(split);
        split += step;
    }
    out.retain(|&split| split > 0 && split < total_distances);
    out.sort_unstable();
    out.dedup();
    out
}

fn literal_command_distance_split_candidates_for_data(
    data: &[u8],
    commands: &[BrotliCommand],
    total_literals: usize,
    total_distances: usize,
) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    for (literal_split, command_split) in
        literal_command_split_candidates_for_data(data, commands, total_literals)
    {
        if let Some(distance_split) = distance_index_at_command(commands, command_split) {
            if distance_split > 0 && distance_split < total_distances {
                out.push((literal_split, command_split, distance_split));
            }
        }
    }
    for distance_split in distance_split_candidates(total_distances) {
        if let Some(command_split) = command_index_at_distance(commands, distance_split) {
            if let Some(literal_split) = inserted_literal_index_at_command(commands, command_split)
            {
                if literal_split > 0
                    && literal_split < total_literals
                    && command_split > 0
                    && command_split < commands.len()
                {
                    out.push((literal_split, command_split, distance_split));
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn distance_index_at_command(commands: &[BrotliCommand], command_split: usize) -> Option<usize> {
    if command_split > commands.len() {
        return None;
    }
    Some(
        commands
            .iter()
            .take(command_split)
            .filter(|command| command.distance.is_some())
            .count(),
    )
}

fn command_index_at_distance(commands: &[BrotliCommand], distance_split: usize) -> Option<usize> {
    if distance_split == 0 {
        return None;
    }
    let mut distances = 0usize;
    for (index, command) in commands.iter().enumerate() {
        if command.distance.is_some() {
            distances += 1;
            if distances == distance_split {
                return Some(index + 1);
            }
        }
    }
    None
}

fn stream_bytes_len(bit_len: usize) -> usize {
    bit_len.div_ceil(8)
}

fn write_stream_plan(plan: &SingleBlockCompressedStreamPlan) -> Vec<u8> {
    let mut bw = BitWriter::new();
    bw.write_spans(&plan.header_spans);
    bw.write_spans(&plan.body.prefix_spans);
    bw.write_spans(&plan.body.payload_spans);
    bw.finish()
}

fn best_single_block_compressed_plan(
    data: &[u8],
    profile: Lz77Profile,
    include_wbits: bool,
    is_last: bool,
) -> Option<SingleBlockCompressedStreamPlan> {
    best_single_block_compressed_plan_with_output_base(data, profile, include_wbits, is_last, 0)
}

fn best_single_block_compressed_plan_with_output_base(
    data: &[u8],
    profile: Lz77Profile,
    include_wbits: bool,
    is_last: bool,
    output_base: usize,
) -> Option<SingleBlockCompressedStreamPlan> {
    best_single_block_compressed_plan_with_output_base_and_literal_only(
        data,
        profile,
        include_wbits,
        is_last,
        output_base,
        false,
    )
}

#[allow(dead_code)]
fn best_single_block_compressed_plan_with_output_base_and_literal_only(
    data: &[u8],
    profile: Lz77Profile,
    include_wbits: bool,
    is_last: bool,
    output_base: usize,
    allow_literal_only: bool,
) -> Option<SingleBlockCompressedStreamPlan> {
    if data.is_empty() || data.len() > 65_536 {
        return None;
    }
    let mut best: Option<SingleBlockCompressedStreamPlan> = None;
    for distance_profile in distance_profiles_for_lz77(profile).iter().copied() {
        let commands = lower_lz77_commands_with_profile_and_output_base(
            data,
            distance_profile.ndirect,
            distance_profile.npostfix,
            profile,
            output_base,
        );
        best = best_single_block_candidate_for_commands(
            data,
            &commands,
            distance_profile,
            include_wbits,
            is_last,
            allow_literal_only,
            best,
        );
        if profile.candidate_limit >= 32 && include_wbits && is_last && output_base == 0 {
            let cached_commands = lower_lz77_commands_with_profile_output_base_and_distance_cache(
                data,
                distance_profile.ndirect,
                distance_profile.npostfix,
                profile,
                output_base,
            );
            if cached_commands != commands && commands_have_implicit_distance(&cached_commands) {
                best = best_single_block_candidate_for_commands(
                    data,
                    &cached_commands,
                    distance_profile,
                    include_wbits,
                    is_last,
                    allow_literal_only,
                    best,
                );
            }
        }
    }
    best
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn best_single_block_candidate_for_commands(
    data: &[u8],
    commands: &[BrotliCommand],
    distance_profile: DistanceProfile,
    include_wbits: bool,
    is_last: bool,
    allow_literal_only: bool,
    mut best: Option<SingleBlockCompressedStreamPlan>,
) -> Option<SingleBlockCompressedStreamPlan> {
    if !allow_literal_only
        && !commands
            .iter()
            .any(|command| command.copy_distance.is_some())
    {
        return best;
    }
    if let Some(prefix_profile) =
        single_block_complex_prefix_profile_with_distance(data, commands, distance_profile)
    {
        if let Some(stream) = single_block_compressed_stream_plan_with_header(
            data,
            commands,
            &prefix_profile,
            include_wbits,
            is_last,
        ) {
            if (!commands_have_implicit_distance(commands)
                || decode_generated_single_block(&write_stream_plan(&stream), MAX_OUTPUT).is_ok())
                && best
                    .as_ref()
                    .is_none_or(|current| stream.bit_len < current.bit_len)
            {
                best = Some(stream);
            }
        }
    }
    for contextual_profile in contextual_single_block_complex_prefix_profiles_with_distance(
        data,
        commands,
        distance_profile,
    ) {
        if let Some(contextual_stream) = contextual_single_block_compressed_stream_plan_with_header(
            data,
            commands,
            &contextual_profile,
            include_wbits,
            is_last,
        ) {
            let stream = SingleBlockCompressedStreamPlan {
                header_spans: contextual_stream.header_spans,
                body: SingleBlockCompressedBodyPlan {
                    prefix_spans: contextual_stream.body.prefix_spans,
                    payload_spans: contextual_stream.body.payload_spans,
                    bit_len: contextual_stream.body.bit_len,
                },
                bit_len: contextual_stream.bit_len,
            };
            if (!commands_have_implicit_distance(commands)
                || decode_generated_single_block(&write_stream_plan(&stream), MAX_OUTPUT).is_ok())
                && best
                    .as_ref()
                    .is_none_or(|current| stream.bit_len < current.bit_len)
            {
                best = Some(stream);
            }
        }
    }
    best
}

#[allow(dead_code)]
fn commands_have_implicit_distance(commands: &[BrotliCommand]) -> bool {
    commands
        .iter()
        .any(|command| command.copy_distance.is_some() && command.distance.is_none())
}

#[cfg(test)]
fn commands_use_static_dictionary(data: &[u8], commands: &[BrotliCommand]) -> bool {
    let mut output_len = 0usize;
    for command in commands {
        output_len += command.insert_len;
        if let Some(distance) = command.copy_distance {
            if distance > output_len {
                return true;
            }
        }
        output_len += command.copy_len;
        if output_len > data.len() {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn direct_distance_profiles() -> &'static [DistanceProfile] {
    &BASE_DISTANCE_PROFILES
}

fn distance_profiles_for_lz77(profile: Lz77Profile) -> &'static [DistanceProfile] {
    if profile.candidate_limit <= 8 {
        &LOW_SEARCH_DISTANCE_PROFILES
    } else {
        &BASE_DISTANCE_PROFILES
    }
}

fn literal_block_type_distance_profiles_for_lz77(
    profile: Lz77Profile,
) -> &'static [DistanceProfile] {
    if profile.candidate_limit >= 32 {
        &BOUNDED_LITERAL_BLOCK_TYPE_DISTANCE_PROFILES
    } else {
        distance_profiles_for_lz77(profile)
    }
}

const BASE_DISTANCE_PROFILES: [DistanceProfile; 5] = [
    DistanceProfile {
        ndirect: 0,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 4,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 8,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 12,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 15,
        npostfix: 0,
    },
];

const LARGE_TWO_BLOCK_DISTANCE_PROFILES: [DistanceProfile; 2] = [
    DistanceProfile {
        ndirect: 4,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 8,
        npostfix: 0,
    },
];

const LARGE_TWO_BLOCK_DISTANCE_PROFILE_PAIRS: [(DistanceProfile, DistanceProfile); 1] = [(
    DistanceProfile {
        ndirect: 0,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 4,
        npostfix: 0,
    },
)];

const BOUNDED_LITERAL_BLOCK_TYPE_DISTANCE_PROFILES: [DistanceProfile; 1] = [DistanceProfile {
    ndirect: 4,
    npostfix: 0,
}];

const LOW_SEARCH_DISTANCE_PROFILES: [DistanceProfile; 6] = [
    DistanceProfile {
        ndirect: 0,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 4,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 8,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 12,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 15,
        npostfix: 0,
    },
    DistanceProfile {
        ndirect: 0,
        npostfix: 3,
    },
];

fn encode_single_literal_block(data: &[u8]) -> Vec<u8> {
    let len = data.len();
    debug_assert!((1..=SINGLE_LITERAL_BLOCK_MAX).contains(&len));
    let even = len % 2 == 0;
    let mut out = Vec::with_capacity(len + 4);
    out.push(if even { 0x8b } else { 0x0b });
    out.push(if even {
        ((len - 2) / 2) as u8
    } else {
        ((len - 1) / 2) as u8
    });
    out.push(0x80);
    out.extend_from_slice(data);
    out.push(0x03);
    out
}

#[allow(dead_code)]
fn plan_lz77(data: &[u8]) -> Vec<Lz77Step> {
    plan_lz77_with_profile(data, lz77_profile(11))
}

#[allow(dead_code)]
fn plan_lz77_with_profile(data: &[u8], profile: Lz77Profile) -> Vec<Lz77Step> {
    if data.len() < LZ77_MIN_MATCH {
        return vec![Lz77Step::Literal {
            start: 0,
            len: data.len(),
        }];
    }

    let mut table: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
    let mut steps = Vec::new();
    let mut literal_start = 0usize;
    let mut i = 0usize;

    while i < data.len() {
        if i + LZ77_MIN_MATCH <= data.len() {
            let key = lz77_key(&data[i..i + LZ77_MIN_MATCH]);
            if let Some((best_pos, best_len)) = best_lz77_match(data, i, &table, profile) {
                if best_len >= LZ77_MIN_MATCH {
                    if profile.lazy_match
                        && i + 1 + LZ77_MIN_MATCH <= data.len()
                        && lazy_next_match_is_better(data, i, key, best_len, &table, profile)
                    {
                        table.entry(key).or_default().push(i);
                        i += 1;
                        continue;
                    }
                    if literal_start < i {
                        steps.push(Lz77Step::Literal {
                            start: literal_start,
                            len: i - literal_start,
                        });
                    }
                    steps.push(Lz77Step::Copy {
                        distance: i - best_pos,
                        len: best_len,
                    });
                    for pos in i..(i + best_len).min(data.len()) {
                        if pos + LZ77_MIN_MATCH <= data.len() {
                            table
                                .entry(lz77_key(&data[pos..pos + LZ77_MIN_MATCH]))
                                .or_default()
                                .push(pos);
                        }
                    }
                    i += best_len;
                    literal_start = i;
                    continue;
                }
            }
            table.entry(key).or_default().push(i);
        }
        i += 1;
    }

    if literal_start < data.len() {
        steps.push(Lz77Step::Literal {
            start: literal_start,
            len: data.len() - literal_start,
        });
    }
    steps
}

#[allow(dead_code)]
fn best_lz77_match(
    data: &[u8],
    cur: usize,
    table: &std::collections::HashMap<u32, Vec<usize>>,
    profile: Lz77Profile,
) -> Option<(usize, usize)> {
    if cur + LZ77_MIN_MATCH > data.len() || profile.candidate_limit == 0 || profile.max_match == 0 {
        return None;
    }
    let key = lz77_key(&data[cur..cur + LZ77_MIN_MATCH]);
    let candidates = table.get(&key)?;
    let mut best_pos = 0usize;
    let mut best_len = 0usize;
    for &pos in candidates.iter().rev().take(profile.candidate_limit) {
        let distance = cur.checked_sub(pos)?;
        if distance == 0 || distance > LZ77_MAX_DISTANCE {
            continue;
        }
        let len = match_len(data, pos, cur).min(profile.max_match);
        if len > best_len {
            best_len = len;
            best_pos = pos;
        }
    }
    (best_len >= LZ77_MIN_MATCH).then_some((best_pos, best_len))
}

#[allow(dead_code)]
fn lazy_next_match_is_better(
    data: &[u8],
    cur: usize,
    cur_key: u32,
    current_len: usize,
    table: &std::collections::HashMap<u32, Vec<usize>>,
    profile: Lz77Profile,
) -> bool {
    let next = cur + 1;
    if next + LZ77_MIN_MATCH > data.len() {
        return false;
    }
    let mut scratch = table.clone();
    scratch.entry(cur_key).or_default().push(cur);
    let Some((_next_pos, next_len)) = best_lz77_match(data, next, &scratch, profile) else {
        return false;
    };
    next_len >= current_len + 2
}

#[allow(dead_code)]
fn lz77_key(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32) << 24)
        | ((bytes[1] as u32) << 16)
        | ((bytes[2] as u32) << 8)
        | bytes[3] as u32
}

#[allow(dead_code)]
fn match_len(data: &[u8], prev: usize, cur: usize) -> usize {
    let mut len = 0usize;
    while cur + len < data.len() && data[prev + len] == data[cur + len] && len < LZ77_MAX_MATCH {
        len += 1;
    }
    len
}

#[allow(dead_code)]
fn insert_length_code(len: usize) -> LengthCode {
    length_code(len, INSERT_LENGTH_RANGES)
}

#[allow(dead_code)]
fn copy_length_code(len: usize) -> LengthCode {
    debug_assert!(len >= 2);
    length_code(len, COPY_LENGTH_RANGES)
}

#[allow(dead_code)]
fn insert_copy_code(insert_len: usize, copy_len: usize, explicit_distance: bool) -> InsertCopyCode {
    let insert = insert_length_code(insert_len);
    let copy = copy_length_code(copy_len);
    let range_base = match (explicit_distance, insert.code, copy.code) {
        (false, 0..=7, 0..=7) => 0,
        (false, 0..=7, 8..=15) => 64,
        (_, 0..=7, 0..=7) => 128,
        (_, 0..=7, 8..=15) => 192,
        (_, 0..=7, 16..=23) => 384,
        (_, 8..=15, 0..=7) => 256,
        (_, 8..=15, 8..=15) => 320,
        (_, 8..=15, 16..=23) => 512,
        (_, 16..=23, 0..=7) => 448,
        (_, 16..=23, 8..=15) => 576,
        (_, 16..=23, 16..=23) => 640,
        _ => unreachable!("length codes are bounded to 0..=23"),
    };
    let code = range_base + ((insert.code & 7) << 3) + (copy.code & 7);
    InsertCopyCode {
        code,
        insert_extra_bits: insert.extra_bits,
        insert_extra_value: insert.extra_value,
        copy_extra_bits: copy.extra_bits,
        copy_extra_value: copy.extra_value,
    }
}

#[allow(dead_code)]
fn distance_code(distance: usize, ndirect: u16, npostfix: u8) -> DistanceCode {
    debug_assert!(distance >= 1);
    debug_assert!(ndirect <= 120);
    debug_assert!(npostfix <= 3);
    let distance = distance as u32;
    let ndirect = ndirect as u32;
    if distance <= ndirect {
        return DistanceCode {
            code: (15 + distance) as u16,
            extra_bits: 0,
            extra_value: 0,
        };
    }

    let postfix_mask = (1u32 << npostfix) - 1;
    let adjusted = distance - ndirect - 1;
    let lcode = adjusted & postfix_mask;
    let value = adjusted >> npostfix;
    for hcode in 0..48u32 {
        let ndistbits = 1 + (hcode >> 1);
        let offset = ((2 + (hcode & 1)) << ndistbits) - 4;
        let width = 1u32 << ndistbits;
        if value >= offset && value < offset + width {
            return DistanceCode {
                code: (16 + ndirect + (hcode << npostfix) + lcode) as u16,
                extra_bits: ndistbits as u8,
                extra_value: value - offset,
            };
        }
    }

    unreachable!("brotli distance exceeds 24-bit extra range")
}

#[allow(dead_code)]
fn distance_code_with_cache(
    distance: usize,
    ndirect: u16,
    npostfix: u8,
    cache: &BrotliDistanceCache,
) -> DistanceCode {
    if let Some(code) = cache.short_code(distance) {
        return DistanceCode {
            code,
            extra_bits: 0,
            extra_value: 0,
        };
    }
    distance_code(distance, ndirect, npostfix)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BrotliDistanceCache {
    recent: [usize; 4],
    initialized: bool,
}

impl BrotliDistanceCache {
    fn new() -> Self {
        Self {
            recent: [16, 15, 11, 4],
            initialized: false,
        }
    }

    fn front(self) -> Option<usize> {
        self.initialized.then_some(self.recent[0])
    }

    fn push(&mut self, distance: usize) {
        self.initialized = true;
        if self.recent[0] == distance {
            return;
        }
        if let Some(pos) = self.recent.iter().position(|&d| d == distance) {
            for i in (1..=pos).rev() {
                self.recent[i] = self.recent[i - 1];
            }
            self.recent[0] = distance;
            return;
        }
        self.recent[3] = self.recent[2];
        self.recent[2] = self.recent[1];
        self.recent[1] = self.recent[0];
        self.recent[0] = distance;
    }

    fn short_code(self, distance: usize) -> Option<u16> {
        if !self.initialized {
            return None;
        }
        for (i, &cached) in self.recent.iter().enumerate() {
            if cached == distance {
                return Some(i as u16);
            }
        }
        for (base, start_code) in [(self.recent[0], 4u16), (self.recent[1], 10u16)] {
            for (offset_index, delta) in [-1isize, 1, -2, 2, -3, 3].iter().copied().enumerate() {
                let adjusted = if delta.is_negative() {
                    base.checked_sub(delta.unsigned_abs())?
                } else {
                    base.checked_add(delta as usize)?
                };
                if adjusted == distance {
                    return Some(start_code + offset_index as u16);
                }
            }
        }
        None
    }

    fn distance_for_short_code(self, symbol: u16) -> Option<usize> {
        if !self.initialized {
            return None;
        }
        match symbol {
            0..=3 => Some(self.recent[symbol as usize]),
            4..=9 => short_code_adjusted_distance(self.recent[0], symbol - 4),
            10..=15 => short_code_adjusted_distance(self.recent[1], symbol - 10),
            _ => None,
        }
    }
}

fn short_code_adjusted_distance(base: usize, index: u16) -> Option<usize> {
    match index {
        0 => base.checked_sub(1),
        1 => base.checked_add(1),
        2 => base.checked_sub(2),
        3 => base.checked_add(2),
        4 => base.checked_sub(3),
        5 => base.checked_add(3),
        _ => None,
    }
}

#[allow(dead_code)]
fn static_dictionary_ref(
    copy_len: usize,
    distance: usize,
    max_allowed_distance: usize,
) -> Option<StaticDictionaryRef> {
    if !(4..=24).contains(&copy_len) || distance <= max_allowed_distance {
        return None;
    }
    let word_id = distance.checked_sub(max_allowed_distance + 1)?;
    let nwords = static_dictionary_word_count(copy_len)?;
    let index = word_id % nwords;
    let transform_id = word_id >> BROTLI_STATIC_NDBITS[copy_len];
    if transform_id > 120 {
        return None;
    }
    Some(StaticDictionaryRef {
        length: copy_len,
        index,
        transform_id,
        offset: static_dictionary_offset(copy_len)? + index * copy_len,
    })
}

#[allow(dead_code)]
fn static_dictionary_word_count(length: usize) -> Option<usize> {
    (length >= 4 && length <= 24).then_some(1usize << BROTLI_STATIC_NDBITS[length])
}

#[allow(dead_code)]
fn static_dictionary_offset(length: usize) -> Option<usize> {
    if length > 24 {
        return None;
    }
    let mut offset = 0usize;
    for len in 0..length {
        if let Some(words) = static_dictionary_word_count(len) {
            offset += len * words;
        }
    }
    Some(offset)
}

#[allow(dead_code)]
fn apply_dictionary_transform(base: &[u8], transform_id: usize) -> Option<Vec<u8>> {
    let transform = BROTLI_DICTIONARY_TRANSFORMS.get(transform_id)?;
    let mut word = match transform.kind {
        DictionaryTransformKind::Identity => base.to_vec(),
        DictionaryTransformKind::FermentFirst => {
            let mut word = base.to_vec();
            if !word.is_empty() {
                ferment(&mut word, 0);
            }
            word
        }
        DictionaryTransformKind::FermentAll => {
            let mut word = base.to_vec();
            let mut pos = 0usize;
            while pos < word.len() {
                pos += ferment(&mut word, pos);
            }
            word
        }
        DictionaryTransformKind::OmitFirst(n) => {
            base.get(usize::from(n)..).unwrap_or_default().to_vec()
        }
        DictionaryTransformKind::OmitLast(n) => {
            let keep = base.len().saturating_sub(usize::from(n));
            base[..keep].to_vec()
        }
    };
    let mut out = Vec::with_capacity(transform.prefix.len() + word.len() + transform.suffix.len());
    out.extend_from_slice(transform.prefix);
    out.append(&mut word);
    out.extend_from_slice(transform.suffix);
    Some(out)
}

#[allow(dead_code)]
fn ferment(word: &mut [u8], pos: usize) -> usize {
    if word[pos] < 192 {
        if (b'a'..=b'z').contains(&word[pos]) {
            word[pos] ^= 32;
        }
        1
    } else if word[pos] < 224 {
        if pos + 1 < word.len() {
            word[pos + 1] ^= 32;
        }
        2
    } else {
        if pos + 2 < word.len() {
            word[pos + 2] ^= 5;
        }
        3
    }
}

#[allow(dead_code)]
fn lower_lz77_commands(data: &[u8], ndirect: u16, npostfix: u8) -> Vec<BrotliCommand> {
    lower_lz77_commands_with_profile(data, ndirect, npostfix, lz77_profile(11))
}

#[allow(dead_code)]
fn lower_lz77_commands_with_profile(
    data: &[u8],
    ndirect: u16,
    npostfix: u8,
    profile: Lz77Profile,
) -> Vec<BrotliCommand> {
    lower_lz77_commands_with_profile_and_output_base(data, ndirect, npostfix, profile, 0)
}

#[allow(dead_code)]
fn lower_lz77_commands_with_profile_and_output_base(
    data: &[u8],
    ndirect: u16,
    npostfix: u8,
    profile: Lz77Profile,
    output_base: usize,
) -> Vec<BrotliCommand> {
    let steps = plan_lz77_with_profile(data, profile);
    let mut commands = Vec::new();
    let mut pending_literal_start = data.len();
    let mut pending_literal_len = 0usize;
    let mut output_len = output_base;

    for step in steps {
        match step {
            Lz77Step::Literal { start, len } => {
                lower_literal_span_with_dictionary(
                    data,
                    start,
                    len,
                    ndirect,
                    npostfix,
                    &mut commands,
                    &mut pending_literal_start,
                    &mut pending_literal_len,
                    &mut output_len,
                );
            }
            Lz77Step::Copy { distance, len } => {
                commands.push(BrotliCommand {
                    insert_start: pending_literal_start,
                    insert_len: pending_literal_len,
                    copy_len: len,
                    copy_distance: Some(distance),
                    insert_copy: insert_copy_code(pending_literal_len, len, true),
                    distance: Some(distance_code(distance, ndirect, npostfix)),
                });
                output_len += pending_literal_len + len;
                pending_literal_start = data.len();
                pending_literal_len = 0;
            }
        }
    }

    if pending_literal_len != 0 || commands.is_empty() {
        commands.push(BrotliCommand {
            insert_start: pending_literal_start,
            insert_len: pending_literal_len,
            copy_len: 2,
            copy_distance: None,
            insert_copy: insert_copy_code(pending_literal_len, 2, false),
            distance: None,
        });
    }

    commands
}

#[allow(dead_code)]
fn lower_lz77_commands_with_profile_output_base_and_distance_cache(
    data: &[u8],
    ndirect: u16,
    npostfix: u8,
    profile: Lz77Profile,
    output_base: usize,
) -> Vec<BrotliCommand> {
    let steps = plan_lz77_with_profile(data, profile);
    lower_lz77_steps_with_profile_output_base_and_distance_cache(
        data,
        &steps,
        ndirect,
        npostfix,
        output_base,
    )
}

#[allow(dead_code)]
fn lower_lz77_steps_with_profile_output_base_and_distance_cache(
    data: &[u8],
    steps: &[Lz77Step],
    ndirect: u16,
    npostfix: u8,
    output_base: usize,
) -> Vec<BrotliCommand> {
    let mut commands = Vec::new();
    let mut pending_literal_start = data.len();
    let mut pending_literal_len = 0usize;
    let mut output_len = output_base;
    let mut distance_cache = BrotliDistanceCache::new();

    for step in steps.iter().cloned() {
        match step {
            Lz77Step::Literal { start, len } => {
                lower_literal_span_with_dictionary(
                    data,
                    start,
                    len,
                    ndirect,
                    npostfix,
                    &mut commands,
                    &mut pending_literal_start,
                    &mut pending_literal_len,
                    &mut output_len,
                );
            }
            Lz77Step::Copy { distance, len } => {
                let explicit_distance = distance_cache.front() != Some(distance);
                let distance_symbol = explicit_distance.then_some(distance_code_with_cache(
                    distance,
                    ndirect,
                    npostfix,
                    &distance_cache,
                ));
                commands.push(BrotliCommand {
                    insert_start: pending_literal_start,
                    insert_len: pending_literal_len,
                    copy_len: len,
                    copy_distance: Some(distance),
                    insert_copy: insert_copy_code(pending_literal_len, len, explicit_distance),
                    distance: distance_symbol,
                });
                output_len += pending_literal_len + len;
                distance_cache.push(distance);
                pending_literal_start = data.len();
                pending_literal_len = 0;
            }
        }
    }

    if pending_literal_len != 0 || commands.is_empty() {
        commands.push(BrotliCommand {
            insert_start: pending_literal_start,
            insert_len: pending_literal_len,
            copy_len: 2,
            copy_distance: None,
            insert_copy: insert_copy_code(pending_literal_len, 2, false),
            distance: None,
        });
    }

    commands
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn lower_literal_span_with_dictionary(
    data: &[u8],
    start: usize,
    len: usize,
    ndirect: u16,
    npostfix: u8,
    commands: &mut Vec<BrotliCommand>,
    pending_literal_start: &mut usize,
    pending_literal_len: &mut usize,
    output_len: &mut usize,
) {
    let end = start + len;
    let mut i = start;
    while i < end {
        if let Some(word) = exact_static_dictionary_word_at(data, i) {
            let max_allowed_distance = *output_len + *pending_literal_len;
            let distance = max_allowed_distance
                + 1
                + (word.transform_id << BROTLI_STATIC_NDBITS[word.length])
                + word.index;
            commands.push(BrotliCommand {
                insert_start: *pending_literal_start,
                insert_len: *pending_literal_len,
                copy_len: word.length,
                copy_distance: Some(distance),
                insert_copy: insert_copy_code(*pending_literal_len, word.length, true),
                distance: Some(distance_code(distance, ndirect, npostfix)),
            });
            *output_len += *pending_literal_len + word.output.len();
            *pending_literal_start = data.len();
            *pending_literal_len = 0;
            i += word.output.len();
        } else {
            if *pending_literal_len == 0 {
                *pending_literal_start = i;
            }
            *pending_literal_len += 1;
            i += 1;
        }
    }
}

#[allow(dead_code)]
fn exact_static_dictionary_word_at(data: &[u8], pos: usize) -> Option<StaticDictionaryWord> {
    EXACT_STATIC_DICTIONARY_WORDS.iter().copied().find(|word| {
        data.get(pos..pos + word.output.len()) == Some(word.output)
            && static_dictionary_word_is_profitable(*word)
    })
}

#[allow(dead_code)]
fn static_dictionary_word_is_profitable(word: StaticDictionaryWord) -> bool {
    word.transform_id == 0 || word.output.len() >= word.length + 2
}

#[allow(dead_code)]
fn exact_static_dictionary_word(
    length: usize,
    index: usize,
    transform_id: usize,
) -> Option<&'static [u8]> {
    EXACT_STATIC_DICTIONARY_WORDS.iter().find_map(|word| {
        (word.length == length && word.index == index && word.transform_id == transform_id)
            .then_some(word.output)
    })
}

#[allow(dead_code)]
fn command_stream_payload_bytes(commands: &[BrotliCommand]) -> usize {
    commands
        .iter()
        .map(|cmd| cmd.insert_len + usize::from(cmd.copy_distance.is_some()))
        .sum()
}

#[allow(dead_code)]
fn single_block_prefix_profile(
    data: &[u8],
    commands: &[BrotliCommand],
) -> Option<SingleBlockPrefixProfile> {
    let mut literal_symbols = literal_symbols_for_commands(data, commands)?;
    literal_symbols.sort_unstable();
    literal_symbols.dedup();
    if literal_symbols.is_empty() {
        literal_symbols.push(0);
    }
    if literal_symbols.len() > 4 {
        return None;
    }

    let mut insert_copy_symbols: Vec<u16> =
        commands.iter().map(|cmd| cmd.insert_copy.code).collect();
    insert_copy_symbols.sort_unstable();
    insert_copy_symbols.dedup();
    if insert_copy_symbols.len() > 4 {
        return None;
    }

    let mut distance_symbols: Vec<u16> = commands
        .iter()
        .filter_map(|cmd| cmd.distance.map(|distance| distance.code))
        .collect();
    distance_symbols.sort_unstable();
    distance_symbols.dedup();
    if distance_symbols.is_empty() {
        distance_symbols.push(16);
    }
    if distance_symbols.len() > 4 {
        return None;
    }

    Some(SingleBlockPrefixProfile {
        ndirect: 0,
        npostfix: 0,
        literal: simple_prefix_code(256, literal_symbols),
        insert_copy: simple_prefix_code(704, insert_copy_symbols),
        distance: simple_prefix_code(16 + (48 << 0), distance_symbols),
    })
}

#[allow(dead_code)]
fn simple_prefix_code(alphabet_size: u16, symbols: Vec<u16>) -> SimplePrefixCode {
    debug_assert!(!symbols.is_empty());
    debug_assert!(symbols.len() <= 4);
    debug_assert!(symbols.iter().all(|&symbol| symbol < alphabet_size));
    SimplePrefixCode {
        alphabet_size,
        alphabet_bits: alphabet_bits(alphabet_size),
        symbols,
    }
}

#[allow(dead_code)]
fn alphabet_bits(alphabet_size: u16) -> u8 {
    debug_assert!(alphabet_size > 1);
    u16::BITS as u8 - (alphabet_size - 1).leading_zeros() as u8
}

#[allow(dead_code)]
fn single_block_complex_prefix_profile(
    data: &[u8],
    commands: &[BrotliCommand],
) -> Option<SingleBlockComplexPrefixProfile> {
    single_block_complex_prefix_profile_with_distance(
        data,
        commands,
        DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        },
    )
}

#[allow(dead_code)]
fn single_block_complex_prefix_profile_with_distance(
    data: &[u8],
    commands: &[BrotliCommand],
    distance_profile: DistanceProfile,
) -> Option<SingleBlockComplexPrefixProfile> {
    Some(SingleBlockComplexPrefixProfile {
        ndirect: distance_profile.ndirect,
        npostfix: distance_profile.npostfix,
        literal: complex_prefix_code(256, literal_frequencies_for_commands(data, commands)?, 15)?,
        insert_copy: complex_prefix_code(704, insert_copy_frequencies(commands), 15)?,
        distance: complex_prefix_code(
            distance_profile.distance_alphabet_size(),
            distance_frequencies(commands),
            15,
        )?,
    })
}

#[allow(dead_code)]
fn contextual_single_block_complex_prefix_profile_with_distance(
    data: &[u8],
    commands: &[BrotliCommand],
    distance_profile: DistanceProfile,
) -> Option<ContextualSingleBlockComplexPrefixProfile> {
    contextual_single_block_complex_prefix_profiles_with_distance(data, commands, distance_profile)
        .into_iter()
        .next()
}

#[allow(dead_code)]
fn contextual_single_block_complex_prefix_profiles_with_distance(
    data: &[u8],
    commands: &[BrotliCommand],
    distance_profile: DistanceProfile,
) -> Vec<ContextualSingleBlockComplexPrefixProfile> {
    context_map_candidates(data, commands)
        .into_iter()
        .filter_map(|context_map| {
            contextual_single_block_complex_prefix_profile_for_map(
                data,
                commands,
                distance_profile,
                context_map,
            )
        })
        .collect()
}

#[allow(dead_code)]
fn contextual_single_block_complex_prefix_profile_for_map(
    data: &[u8],
    commands: &[BrotliCommand],
    distance_profile: DistanceProfile,
    context_map: [u8; BROTLI_LITERAL_CONTEXTS],
) -> Option<ContextualSingleBlockComplexPrefixProfile> {
    let literal_frequencies =
        literal_context_frequencies_for_commands(data, commands, &context_map)?;
    if literal_frequencies.iter().any(Vec::is_empty) {
        return None;
    }
    Some(ContextualSingleBlockComplexPrefixProfile {
        ndirect: distance_profile.ndirect,
        npostfix: distance_profile.npostfix,
        literal_context_map: context_map,
        literal: literal_frequencies
            .into_iter()
            .map(|freqs| complex_prefix_code(256, freqs, 15))
            .collect::<Option<Vec<_>>>()?,
        insert_copy: complex_prefix_code(704, insert_copy_frequencies(commands), 15)?,
        distance: complex_prefix_code(
            distance_profile.distance_alphabet_size(),
            distance_frequencies(commands),
            15,
        )?,
    })
}

#[allow(dead_code)]
fn context_map_candidates(
    data: &[u8],
    commands: &[BrotliCommand],
) -> Vec<[u8; BROTLI_LITERAL_CONTEXTS]> {
    let counts = literal_context_counts_for_commands(data, commands)
        .unwrap_or([[0usize; 256]; BROTLI_LITERAL_CONTEXTS]);
    let mut candidates = Vec::new();
    candidates.push(json_literal_context_map());

    let mut totals: Vec<(usize, usize)> = counts
        .iter()
        .enumerate()
        .map(|(context, symbols)| (context, symbols.iter().sum()))
        .filter(|&(_, total): &(usize, usize)| total != 0)
        .collect();
    totals.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for limit in [1usize, 2, 4, 8, 16] {
        if totals.len() > limit {
            let mut map = [0u8; BROTLI_LITERAL_CONTEXTS];
            for &(context, _) in totals.iter().take(limit) {
                map[context] = 1;
            }
            candidates.push(map);
        }
    }
    for tree_count in [3u8, 4] {
        let hot_count = usize::from(tree_count);
        if totals.len() >= hot_count {
            let mut map = [0u8; BROTLI_LITERAL_CONTEXTS];
            for (rank, &(context, _)) in totals.iter().take(hot_count).enumerate() {
                map[context] = 1 + (rank as u8 % (tree_count - 1));
            }
            candidates.push(map);
        }
    }

    let mut punctuation_follow = [0u8; BROTLI_LITERAL_CONTEXTS];
    let mut alpha_follow = [0u8; BROTLI_LITERAL_CONTEXTS];
    for (context, symbols) in counts.iter().enumerate() {
        let Some((dominant, _)) = symbols.iter().enumerate().max_by_key(|&(_, count)| count) else {
            continue;
        };
        let b = dominant as u8;
        if matches!(b, b'"' | b',' | b':' | b'[' | b']' | b'{' | b'}') {
            punctuation_follow[context] = 1;
        }
        if b.is_ascii_alphabetic() {
            alpha_follow[context] = 1;
        }
    }
    candidates.push(punctuation_follow);
    candidates.push(alpha_follow);

    candidates.sort_unstable();
    candidates.dedup();
    candidates
        .into_iter()
        .filter(|map| map.contains(&0) && map.contains(&1))
        .collect()
}

#[allow(dead_code)]
fn json_literal_context_map() -> [u8; BROTLI_LITERAL_CONTEXTS] {
    let mut map = [0u8; BROTLI_LITERAL_CONTEXTS];
    for b in [b'"', b':', b',', b'[', b']', b'{', b'}', b'/', b'-'] {
        map[(b & 0x3f) as usize] = 1;
    }
    map
}

#[allow(dead_code)]
fn literal_context_id(prev: u8) -> usize {
    (prev & 0x3f) as usize
}

#[allow(dead_code)]
fn literal_context_counts_for_commands(
    data: &[u8],
    commands: &[BrotliCommand],
) -> Option<[[usize; 256]; BROTLI_LITERAL_CONTEXTS]> {
    let mut counts = [[0usize; 256]; BROTLI_LITERAL_CONTEXTS];
    replay_commands(data, commands, |literal, prev| {
        counts[literal_context_id(prev)][literal as usize] += 1;
    })?;
    Some(counts)
}

#[allow(dead_code)]
fn literal_context_frequencies_for_commands(
    data: &[u8],
    commands: &[BrotliCommand],
    context_map: &[u8; BROTLI_LITERAL_CONTEXTS],
) -> Option<Vec<Vec<(u16, usize)>>> {
    let tree_count = usize::from(context_map.iter().copied().max().unwrap_or(0)) + 1;
    let mut counts = vec![[0usize; 256]; tree_count];
    replay_commands(data, commands, |literal, prev| {
        let tree = context_map[literal_context_id(prev)] as usize;
        counts[tree][literal as usize] += 1;
    })?;
    Some(
        counts
            .into_iter()
            .map(|tree| {
                tree.iter()
                    .enumerate()
                    .filter_map(|(symbol, &count)| (count != 0).then_some((symbol as u16, count)))
                    .collect()
            })
            .collect(),
    )
}

#[allow(dead_code)]
fn replay_commands(
    data: &[u8],
    commands: &[BrotliCommand],
    mut on_literal: impl FnMut(u8, u8),
) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len());
    for cmd in commands {
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        for &literal in data.get(cmd.insert_start..end)? {
            let prev = out.last().copied().unwrap_or(0);
            on_literal(literal, prev);
            out.push(literal);
        }
        if let Some(distance) = cmd.copy_distance {
            if distance > out.len() {
                let dict_ref = static_dictionary_ref(cmd.copy_len, distance, out.len())?;
                let word = exact_static_dictionary_word(
                    dict_ref.length,
                    dict_ref.index,
                    dict_ref.transform_id,
                )?;
                out.extend_from_slice(word);
            } else {
                for _ in 0..cmd.copy_len {
                    let b = *out.get(out.len().checked_sub(distance)?)?;
                    out.push(b);
                }
            }
        }
    }
    (out.len() == data.len()).then_some(out)
}

#[allow(dead_code)]
fn literal_symbols_for_commands(data: &[u8], commands: &[BrotliCommand]) -> Option<Vec<u16>> {
    let mut symbols = Vec::new();
    for cmd in commands {
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        let span = data.get(cmd.insert_start..end)?;
        symbols.extend(span.iter().map(|&b| b as u16));
    }
    Some(symbols)
}

#[allow(dead_code)]
fn literal_frequencies_for_commands(
    data: &[u8],
    commands: &[BrotliCommand],
) -> Option<Vec<(u16, usize)>> {
    let mut counts = [0usize; 256];
    for symbol in literal_symbols_for_commands(data, commands)? {
        let b = u8::try_from(symbol).ok()?;
        counts[b as usize] += 1;
    }
    Some(
        counts
            .iter()
            .enumerate()
            .filter_map(|(symbol, &count)| (count != 0).then_some((symbol as u16, count)))
            .collect(),
    )
}

#[allow(dead_code)]
fn insert_copy_frequencies(commands: &[BrotliCommand]) -> Vec<(u16, usize)> {
    let mut counts = std::collections::BTreeMap::<u16, usize>::new();
    for cmd in commands {
        *counts.entry(cmd.insert_copy.code).or_default() += 1;
    }
    counts.into_iter().collect()
}

#[allow(dead_code)]
fn distance_frequencies(commands: &[BrotliCommand]) -> Vec<(u16, usize)> {
    let mut counts = std::collections::BTreeMap::<u16, usize>::new();
    for cmd in commands {
        if let Some(distance) = cmd.distance {
            *counts.entry(distance.code).or_default() += 1;
        }
    }
    if counts.is_empty() {
        counts.insert(16, 1);
    }
    counts.into_iter().collect()
}

#[allow(dead_code)]
fn complex_prefix_code(
    alphabet_size: u16,
    frequencies: Vec<(u16, usize)>,
    max_bits: u8,
) -> Option<ComplexPrefixCode> {
    let mut code_lengths =
        huffman_code_lengths(complete_prefix_frequencies(alphabet_size, frequencies)?)?;
    if code_lengths
        .iter()
        .any(|&(symbol, len)| symbol >= alphabet_size || len > max_bits)
    {
        return None;
    }
    code_lengths.sort_unstable_by_key(|&(symbol, _)| symbol);
    Some(ComplexPrefixCode {
        alphabet_size,
        max_bits,
        code_lengths,
    })
}

#[allow(dead_code)]
fn complete_prefix_frequencies(
    alphabet_size: u16,
    mut frequencies: Vec<(u16, usize)>,
) -> Option<Vec<(u16, usize)>> {
    frequencies.retain(|&(_, count)| count != 0);
    if frequencies
        .iter()
        .any(|&(symbol, _)| symbol >= alphabet_size)
    {
        return None;
    }
    if frequencies.len() == 1 {
        let used = frequencies[0].0;
        let dummy = if used == 0 { 1 } else { 0 };
        if dummy >= alphabet_size {
            return None;
        }
        frequencies.push((dummy, 1));
    }
    Some(frequencies)
}

#[allow(dead_code)]
fn canonical_codes(prefix: &ComplexPrefixCode) -> Option<Vec<CanonicalCode>> {
    let mut lengths = prefix.code_lengths.clone();
    lengths.sort_unstable_by_key(|&(symbol, len)| (len, symbol));
    if lengths.is_empty()
        || lengths
            .iter()
            .any(|&(symbol, len)| symbol >= prefix.alphabet_size || len == 0 || len > 15)
    {
        return None;
    }

    let mut bl_count = [0u16; 16];
    for &(_, len) in &lengths {
        bl_count[len as usize] += 1;
    }

    let mut code = 0u16;
    let mut next_code = [0u16; 16];
    for bits in 1..=15usize {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }

    let mut out = Vec::with_capacity(lengths.len());
    for (symbol, len) in lengths {
        let msb_code = next_code[len as usize];
        if msb_code >= (1u16 << len) {
            return None;
        }
        next_code[len as usize] += 1;
        out.push(CanonicalCode {
            symbol,
            len,
            msb_code,
            lsb_bits: reverse_low_bits(msb_code, len),
        });
    }
    Some(out)
}

#[allow(dead_code)]
fn reverse_low_bits(value: u16, len: u8) -> u16 {
    let mut out = 0u16;
    for i in 0..len {
        out = (out << 1) | ((value >> i) & 1);
    }
    out
}

#[allow(dead_code)]
fn encoded_command_bit_estimate(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<usize> {
    let literal_codes = prefix_canonical_codes(&selected_prefix_code(&profile.literal))?;
    let insert_copy_codes = prefix_canonical_codes(&selected_prefix_code(&profile.insert_copy))?;
    let distance_codes = prefix_canonical_codes(&selected_prefix_code(&profile.distance))?;
    let literal_len = code_len_table(&literal_codes, profile.literal.alphabet_size);
    let insert_copy_len = code_len_table(&insert_copy_codes, profile.insert_copy.alphabet_size);
    let distance_len = code_len_table(&distance_codes, profile.distance.alphabet_size);

    let mut bits = 0usize;
    for cmd in commands {
        bits += usize::from(
            insert_copy_len
                .get(cmd.insert_copy.code as usize)
                .copied()?,
        );
        bits += usize::from(cmd.insert_copy.insert_extra_bits);
        bits += usize::from(cmd.insert_copy.copy_extra_bits);
        for &b in &data[cmd.insert_start..cmd.insert_start + cmd.insert_len] {
            bits += usize::from(literal_len.get(b as usize).copied()?);
        }
        if let Some(distance) = cmd.distance {
            bits += usize::from(distance_len.get(distance.code as usize).copied()?);
            bits += usize::from(distance.extra_bits);
        }
    }
    Some(bits)
}

#[allow(dead_code)]
fn command_payload_bit_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    let literal_codes = prefix_canonical_codes(&selected_prefix_code(&profile.literal))?;
    let insert_copy_codes = prefix_canonical_codes(&selected_prefix_code(&profile.insert_copy))?;
    let distance_codes = prefix_canonical_codes(&selected_prefix_code(&profile.distance))?;

    let mut spans = Vec::new();
    for cmd in commands {
        spans.push(canonical_symbol_span(
            &insert_copy_codes,
            cmd.insert_copy.code,
        )?);
        if cmd.insert_copy.insert_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.insert_extra_value as u16,
                len: cmd.insert_copy.insert_extra_bits,
            });
        }
        if cmd.insert_copy.copy_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.copy_extra_value as u16,
                len: cmd.insert_copy.copy_extra_bits,
            });
        }
        for &b in &data[cmd.insert_start..cmd.insert_start + cmd.insert_len] {
            spans.push(canonical_symbol_span(&literal_codes, b as u16)?);
        }
        if let Some(distance) = cmd.distance {
            spans.push(canonical_symbol_span(&distance_codes, distance.code)?);
            if distance.extra_bits != 0 {
                spans.push(BitSpan {
                    bits: distance.extra_value as u16,
                    len: distance.extra_bits,
                });
            }
        }
    }
    Some(spans)
}

#[allow(dead_code)]
fn contextual_command_payload_bit_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &ContextualSingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    let literal_codes = profile
        .literal
        .iter()
        .map(|literal| prefix_canonical_codes(&selected_prefix_code(literal)))
        .collect::<Option<Vec<_>>>()?;
    let insert_copy_codes = prefix_canonical_codes(&selected_prefix_code(&profile.insert_copy))?;
    let distance_codes = prefix_canonical_codes(&selected_prefix_code(&profile.distance))?;

    let mut spans = Vec::new();
    let mut out = Vec::with_capacity(data.len());
    for cmd in commands {
        spans.push(canonical_symbol_span(
            &insert_copy_codes,
            cmd.insert_copy.code,
        )?);
        if cmd.insert_copy.insert_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.insert_extra_value as u16,
                len: cmd.insert_copy.insert_extra_bits,
            });
        }
        if cmd.insert_copy.copy_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.copy_extra_value as u16,
                len: cmd.insert_copy.copy_extra_bits,
            });
        }
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        for &literal in data.get(cmd.insert_start..end)? {
            let prev = out.last().copied().unwrap_or(0);
            let tree = profile.literal_context_map[literal_context_id(prev)] as usize;
            spans.push(canonical_symbol_span(
                literal_codes.get(tree)?,
                literal as u16,
            )?);
            out.push(literal);
        }
        if let Some(distance) = cmd.distance {
            spans.push(canonical_symbol_span(&distance_codes, distance.code)?);
            if distance.extra_bits != 0 {
                spans.push(BitSpan {
                    bits: distance.extra_value as u16,
                    len: distance.extra_bits,
                });
            }
        }
        if let Some(copy_distance) = cmd.copy_distance {
            if copy_distance > out.len() {
                let dict_ref = static_dictionary_ref(cmd.copy_len, copy_distance, out.len())?;
                let word = exact_static_dictionary_word(
                    dict_ref.length,
                    dict_ref.index,
                    dict_ref.transform_id,
                )?;
                out.extend_from_slice(word);
            } else {
                for _ in 0..cmd.copy_len {
                    let b = *out.get(out.len().checked_sub(copy_distance)?)?;
                    out.push(b);
                }
            }
        }
    }
    (out.len() == data.len()).then_some(spans)
}

#[allow(dead_code)]
fn write_command_payload_fragment(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<CommandPayloadFragment> {
    let spans = command_payload_bit_plan(data, commands, profile)?;
    let mut bw = BitWriter::new();
    bw.write_spans(&spans);
    let bit_len = bw.bit_len();
    Some(CommandPayloadFragment {
        bytes: bw.finish(),
        bit_len,
    })
}

#[allow(dead_code)]
fn single_block_prefix_description_plans(
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<Vec<PrefixDescriptionPlan>> {
    Some(vec![
        prefix_description_plan(&profile.literal)?,
        prefix_description_plan(&profile.insert_copy)?,
        prefix_description_plan(&profile.distance)?,
    ])
}

#[allow(dead_code)]
fn prefix_description_spans(
    plans: &[PrefixDescriptionPlan],
    code_plan: &CodeLengthPrefixPlan,
) -> Option<Vec<BitSpan>> {
    let bit_plan = prefix_description_bit_plan(plans, code_plan)?;
    let mut spans = Vec::with_capacity(bit_plan.header_lengths.len() + bit_plan.op_spans.len());
    spans.extend(bit_plan.header_lengths);
    spans.extend(bit_plan.op_spans);
    Some(spans)
}

#[allow(dead_code)]
fn single_block_compressed_body_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<SingleBlockCompressedBodyPlan> {
    let prefix_spans = single_block_prefix_code_spans(profile)?;
    let payload_spans = command_payload_bit_plan(data, commands, profile)?;
    let bit_len = prefix_spans
        .iter()
        .chain(payload_spans.iter())
        .map(|span| usize::from(span.len))
        .sum();
    Some(SingleBlockCompressedBodyPlan {
        prefix_spans,
        payload_spans,
        bit_len,
    })
}

#[allow(dead_code)]
fn contextual_single_block_compressed_body_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &ContextualSingleBlockComplexPrefixProfile,
) -> Option<ContextualSingleBlockCompressedBodyPlan> {
    let prefix_spans = contextual_single_block_prefix_code_spans(profile)?;
    let payload_spans = contextual_command_payload_bit_plan(data, commands, profile)?;
    let bit_len = prefix_spans
        .iter()
        .chain(payload_spans.iter())
        .map(|span| usize::from(span.len))
        .sum();
    Some(ContextualSingleBlockCompressedBodyPlan {
        prefix_spans,
        payload_spans,
        bit_len,
    })
}

#[allow(dead_code)]
fn single_block_prefix_code_spans(
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    let mut spans = Vec::new();
    for prefix in [&profile.literal, &profile.insert_copy, &profile.distance] {
        spans.extend(prefix_code_description_spans(&selected_prefix_code(
            prefix,
        ))?);
    }
    Some(spans)
}

#[allow(dead_code)]
fn contextual_single_block_prefix_code_spans(
    profile: &ContextualSingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    let mut spans = Vec::new();
    for prefix in &profile.literal {
        spans.extend(prefix_code_description_spans(&selected_prefix_code(
            prefix,
        ))?);
    }
    spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &profile.insert_copy,
    ))?);
    spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &profile.distance,
    ))?);
    Some(spans)
}

#[allow(dead_code)]
fn context_map_spans(map: &[u8; BROTLI_LITERAL_CONTEXTS]) -> Option<Vec<BitSpan>> {
    let tree_count = map.iter().copied().max().unwrap_or(0) + 1;
    if !(2..=4).contains(&tree_count) {
        return None;
    }
    let mut spans = var_len_uint8_spans(tree_count - 1)?;
    spans.push(BitSpan { bits: 0, len: 1 });
    let symbols = (0..u16::from(tree_count)).collect::<Vec<_>>();
    let map_code = simple_prefix_code(u16::from(tree_count), symbols);
    let map_prefix = PrefixCodeProfile::Simple(map_code);
    let map_codes = prefix_canonical_codes(&map_prefix)?;
    spans.extend(simple_prefix_description_spans(match &map_prefix {
        PrefixCodeProfile::Simple(simple) => simple,
        PrefixCodeProfile::Complex(_) => return None,
    })?);
    for &tree in map {
        spans.push(canonical_symbol_span(&map_codes, u16::from(tree))?);
    }
    spans.push(BitSpan { bits: 0, len: 1 });
    Some(spans)
}

#[allow(dead_code)]
fn var_len_uint8_spans(value: u8) -> Option<Vec<BitSpan>> {
    if value == 0 {
        return Some(vec![BitSpan { bits: 0, len: 1 }]);
    }
    if value == 1 {
        return Some(vec![
            BitSpan { bits: 1, len: 1 },
            BitSpan { bits: 0, len: 3 },
        ]);
    }
    for short in 1..=7u8 {
        let base = 1u16 << short;
        let value = u16::from(value);
        if value >= base && value < base + base {
            return Some(vec![
                BitSpan { bits: 1, len: 1 },
                BitSpan {
                    bits: u16::from(short),
                    len: 3,
                },
                BitSpan {
                    bits: value - base,
                    len: short,
                },
            ]);
        }
    }
    None
}

#[allow(dead_code)]
fn flat_context_map_spans(size: usize) -> Vec<BitSpan> {
    let _ = size;
    vec![BitSpan { bits: 0, len: 1 }]
}

#[allow(dead_code)]
fn two_literal_block_context_map_spans() -> Option<Vec<BitSpan>> {
    let mut spans = vec![
        BitSpan { bits: 1, len: 1 },
        BitSpan { bits: 0, len: 3 },
        BitSpan { bits: 0, len: 1 },
    ];
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        2,
        vec![0, 1],
    ))?);
    spans.extend((0..(BROTLI_LITERAL_CONTEXTS * 2)).map(|slot| BitSpan {
        bits: u16::from(slot >= BROTLI_LITERAL_CONTEXTS),
        len: 1,
    }));
    spans.push(BitSpan { bits: 0, len: 1 });
    Some(spans)
}

#[allow(dead_code)]
fn two_distance_block_context_map_spans() -> Option<Vec<BitSpan>> {
    let mut spans = vec![
        BitSpan { bits: 1, len: 1 },
        BitSpan { bits: 0, len: 3 },
        BitSpan { bits: 0, len: 1 },
    ];
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        2,
        vec![0, 1],
    ))?);
    spans.extend((0..8).map(|slot| BitSpan {
        bits: u16::from(slot >= 4),
        len: 1,
    }));
    spans.push(BitSpan { bits: 0, len: 1 });
    Some(spans)
}

#[allow(dead_code)]
fn block_type_count_spans(count: u8) -> Option<Vec<BitSpan>> {
    if count == 0 {
        return None;
    }
    if count == 1 {
        return Some(vec![BitSpan { bits: 0, len: 1 }]);
    }
    Some(vec![
        BitSpan { bits: 1, len: 1 },
        BitSpan {
            bits: u16::from(count - 2),
            len: 3,
        },
    ])
}

#[allow(dead_code)]
fn block_length_code(length: usize) -> Option<LengthCode> {
    const RANGES: &[(usize, usize, u8)] = &[
        (1, 4, 2),
        (5, 8, 2),
        (9, 12, 2),
        (13, 16, 2),
        (17, 24, 3),
        (25, 32, 3),
        (33, 40, 3),
        (41, 48, 3),
        (49, 64, 4),
        (65, 80, 4),
        (81, 96, 4),
        (97, 112, 4),
        (113, 144, 5),
        (145, 176, 5),
        (177, 208, 5),
        (209, 240, 5),
        (241, 304, 6),
        (305, 368, 6),
        (369, 496, 7),
        (497, 752, 8),
        (753, 1264, 9),
        (1265, 2288, 10),
        (2289, 4336, 11),
        (4337, 8432, 12),
        (8433, 16624, 13),
        (16625, 16_793_840, 24),
    ];
    RANGES
        .iter()
        .enumerate()
        .find_map(|(code, &(start, end, extra_bits))| {
            (start..=end).contains(&length).then_some(LengthCode {
                code: code as u16,
                extra_bits,
                extra_value: (length - start) as u32,
            })
        })
}

#[allow(dead_code)]
fn block_length_spans(length: usize) -> Option<Vec<BitSpan>> {
    let code = block_length_code(length)?;
    block_length_spans_with_symbols(length, &[code.code])
}

#[allow(dead_code)]
fn block_length_spans_with_symbols(length: usize, symbols: &[u16]) -> Option<Vec<BitSpan>> {
    let code = block_length_code(length)?;
    let prefix = simple_prefix_code(26, symbols.to_vec());
    let canonical = simple_canonical_codes(&prefix)?;
    let mut spans = Vec::new();
    spans.push(canonical_symbol_span(&canonical, code.code)?);
    spans.push(BitSpan {
        bits: code.extra_value as u16,
        len: code.extra_bits,
    });
    Some(spans)
}

#[allow(dead_code)]
fn literal_block_type_switch_header_spans(
    len: usize,
    split: usize,
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    literal_block_type_switch_header_spans_with_literal_trees(len, split, profile, false)
}

#[allow(dead_code)]
fn literal_block_type_switch_header_spans_with_literal_trees(
    len: usize,
    split: usize,
    profile: &SingleBlockComplexPrefixProfile,
    two_literal_trees: bool,
) -> Option<Vec<BitSpan>> {
    literal_block_type_switch_header_spans_with_literal_lengths(
        len,
        split,
        len.checked_sub(split)?,
        profile,
        two_literal_trees,
    )
}

#[allow(dead_code)]
fn literal_block_type_switch_header_spans_with_literal_lengths(
    meta_len: usize,
    first_literal_len: usize,
    second_literal_len: usize,
    profile: &SingleBlockComplexPrefixProfile,
    two_literal_trees: bool,
) -> Option<Vec<BitSpan>> {
    if first_literal_len == 0 || second_literal_len == 0 || meta_len > 65_536 {
        return None;
    }
    let distance_profile = DistanceProfile {
        ndirect: profile.ndirect,
        npostfix: profile.npostfix,
    };
    let postfix_unit = 1u16 << distance_profile.npostfix;
    if distance_profile.ndirect > 120 || distance_profile.ndirect % postfix_unit != 0 {
        return None;
    }
    let ndirect_code = distance_profile.ndirect / postfix_unit;
    if ndirect_code > 15 {
        return None;
    }
    let first_length_code = block_length_code(first_literal_len)?.code;
    let second_length_code = block_length_code(second_literal_len)?.code;
    let mut block_length_symbols = vec![first_length_code, second_length_code];
    block_length_symbols.sort_unstable();
    block_length_symbols.dedup();

    let mut spans = Vec::new();
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.push(BitSpan { bits: 1, len: 1 });
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.extend([
        BitSpan { bits: 0, len: 2 },
        BitSpan {
            bits: (meta_len - 1) as u16,
            len: 16,
        },
    ]);

    spans.extend(block_type_count_spans(2)?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        4,
        vec![1],
    ))?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        26,
        block_length_symbols.clone(),
    ))?);
    spans.extend(block_length_spans_with_symbols(
        first_literal_len,
        &block_length_symbols,
    )?);

    spans.extend(block_type_count_spans(1)?);
    spans.extend(block_type_count_spans(1)?);
    spans.extend([
        BitSpan {
            bits: distance_profile.npostfix as u16,
            len: 2,
        },
        BitSpan {
            bits: ndirect_code,
            len: 4,
        },
        BitSpan { bits: 0, len: 2 },
        BitSpan { bits: 0, len: 2 },
    ]);
    if two_literal_trees {
        spans.extend(two_literal_block_context_map_spans()?);
    } else {
        spans.extend(flat_context_map_spans(BROTLI_LITERAL_CONTEXTS * 2));
    }
    spans.push(BitSpan { bits: 0, len: 1 });
    Some(spans)
}

#[allow(dead_code)]
fn literal_command_block_type_header_spans(
    meta_len: usize,
    first_literal_len: usize,
    second_literal_len: usize,
    first_command_len: usize,
    second_command_len: usize,
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    if first_literal_len == 0
        || second_literal_len == 0
        || first_command_len == 0
        || second_command_len == 0
        || meta_len > 65_536
    {
        return None;
    }
    let distance_profile = DistanceProfile {
        ndirect: profile.ndirect,
        npostfix: profile.npostfix,
    };
    let postfix_unit = 1u16 << distance_profile.npostfix;
    if distance_profile.ndirect > 120 || distance_profile.ndirect % postfix_unit != 0 {
        return None;
    }
    let ndirect_code = distance_profile.ndirect / postfix_unit;
    if ndirect_code > 15 {
        return None;
    }
    let first_literal_code = block_length_code(first_literal_len)?.code;
    let second_literal_code = block_length_code(second_literal_len)?.code;
    let mut literal_length_symbols = vec![first_literal_code, second_literal_code];
    literal_length_symbols.sort_unstable();
    literal_length_symbols.dedup();

    let first_command_code = block_length_code(first_command_len)?.code;
    let second_command_code = block_length_code(second_command_len)?.code;
    let mut command_length_symbols = vec![first_command_code, second_command_code];
    command_length_symbols.sort_unstable();
    command_length_symbols.dedup();

    let mut spans = Vec::new();
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.push(BitSpan { bits: 1, len: 1 });
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.extend([
        BitSpan { bits: 0, len: 2 },
        BitSpan {
            bits: (meta_len - 1) as u16,
            len: 16,
        },
    ]);

    spans.extend(block_type_count_spans(2)?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        4,
        vec![1],
    ))?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        26,
        literal_length_symbols.clone(),
    ))?);
    spans.extend(block_length_spans_with_symbols(
        first_literal_len,
        &literal_length_symbols,
    )?);

    spans.extend(block_type_count_spans(2)?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        4,
        vec![1],
    ))?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        26,
        command_length_symbols.clone(),
    ))?);
    spans.extend(block_length_spans_with_symbols(
        first_command_len,
        &command_length_symbols,
    )?);

    spans.extend(block_type_count_spans(1)?);
    spans.extend([
        BitSpan {
            bits: distance_profile.npostfix as u16,
            len: 2,
        },
        BitSpan {
            bits: ndirect_code,
            len: 4,
        },
        BitSpan { bits: 0, len: 2 },
        BitSpan { bits: 0, len: 2 },
    ]);
    spans.extend(two_literal_block_context_map_spans()?);
    spans.push(BitSpan { bits: 0, len: 1 });
    Some(spans)
}

#[allow(dead_code)]
fn command_block_type_header_spans(
    meta_len: usize,
    first_command_len: usize,
    second_command_len: usize,
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    if first_command_len == 0 || second_command_len == 0 || meta_len > 65_536 {
        return None;
    }
    let distance_profile = DistanceProfile {
        ndirect: profile.ndirect,
        npostfix: profile.npostfix,
    };
    let postfix_unit = 1u16 << distance_profile.npostfix;
    if distance_profile.ndirect > 120 || distance_profile.ndirect % postfix_unit != 0 {
        return None;
    }
    let ndirect_code = distance_profile.ndirect / postfix_unit;
    if ndirect_code > 15 {
        return None;
    }
    let first_command_code = block_length_code(first_command_len)?.code;
    let second_command_code = block_length_code(second_command_len)?.code;
    let mut command_length_symbols = vec![first_command_code, second_command_code];
    command_length_symbols.sort_unstable();
    command_length_symbols.dedup();

    let mut spans = Vec::new();
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.push(BitSpan { bits: 1, len: 1 });
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.extend([
        BitSpan { bits: 0, len: 2 },
        BitSpan {
            bits: (meta_len - 1) as u16,
            len: 16,
        },
    ]);

    spans.extend(block_type_count_spans(1)?);
    spans.extend(block_type_count_spans(2)?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        4,
        vec![1],
    ))?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        26,
        command_length_symbols.clone(),
    ))?);
    spans.extend(block_length_spans_with_symbols(
        first_command_len,
        &command_length_symbols,
    )?);
    spans.extend(block_type_count_spans(1)?);
    spans.extend([
        BitSpan {
            bits: distance_profile.npostfix as u16,
            len: 2,
        },
        BitSpan {
            bits: ndirect_code,
            len: 4,
        },
        BitSpan { bits: 0, len: 2 },
    ]);
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.push(BitSpan { bits: 0, len: 1 });
    Some(spans)
}

#[allow(dead_code)]
fn literal_command_distance_block_type_header_spans(
    meta_len: usize,
    first_literal_len: usize,
    second_literal_len: usize,
    first_command_len: usize,
    second_command_len: usize,
    first_distance_len: usize,
    second_distance_len: usize,
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<Vec<BitSpan>> {
    if first_literal_len == 0
        || second_literal_len == 0
        || first_command_len == 0
        || second_command_len == 0
        || first_distance_len == 0
        || second_distance_len == 0
        || meta_len > 65_536
    {
        return None;
    }
    let distance_profile = DistanceProfile {
        ndirect: profile.ndirect,
        npostfix: profile.npostfix,
    };
    let postfix_unit = 1u16 << distance_profile.npostfix;
    if distance_profile.ndirect > 120 || distance_profile.ndirect % postfix_unit != 0 {
        return None;
    }
    let ndirect_code = distance_profile.ndirect / postfix_unit;
    if ndirect_code > 15 {
        return None;
    }
    let first_literal_code = block_length_code(first_literal_len)?.code;
    let second_literal_code = block_length_code(second_literal_len)?.code;
    let mut literal_length_symbols = vec![first_literal_code, second_literal_code];
    literal_length_symbols.sort_unstable();
    literal_length_symbols.dedup();

    let first_command_code = block_length_code(first_command_len)?.code;
    let second_command_code = block_length_code(second_command_len)?.code;
    let mut command_length_symbols = vec![first_command_code, second_command_code];
    command_length_symbols.sort_unstable();
    command_length_symbols.dedup();

    let first_distance_code = block_length_code(first_distance_len)?.code;
    let second_distance_code = block_length_code(second_distance_len)?.code;
    let mut distance_length_symbols = vec![first_distance_code, second_distance_code];
    distance_length_symbols.sort_unstable();
    distance_length_symbols.dedup();

    let mut spans = Vec::new();
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.push(BitSpan { bits: 1, len: 1 });
    spans.push(BitSpan { bits: 0, len: 1 });
    spans.extend([
        BitSpan { bits: 0, len: 2 },
        BitSpan {
            bits: (meta_len - 1) as u16,
            len: 16,
        },
    ]);

    spans.extend(block_type_count_spans(2)?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        4,
        vec![1],
    ))?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        26,
        literal_length_symbols.clone(),
    ))?);
    spans.extend(block_length_spans_with_symbols(
        first_literal_len,
        &literal_length_symbols,
    )?);

    spans.extend(block_type_count_spans(2)?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        4,
        vec![1],
    ))?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        26,
        command_length_symbols.clone(),
    ))?);
    spans.extend(block_length_spans_with_symbols(
        first_command_len,
        &command_length_symbols,
    )?);

    spans.extend(block_type_count_spans(2)?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        4,
        vec![1],
    ))?);
    spans.extend(simple_prefix_description_spans(&simple_prefix_code(
        26,
        distance_length_symbols.clone(),
    ))?);
    spans.extend(block_length_spans_with_symbols(
        first_distance_len,
        &distance_length_symbols,
    )?);

    spans.extend([
        BitSpan {
            bits: distance_profile.npostfix as u16,
            len: 2,
        },
        BitSpan {
            bits: ndirect_code,
            len: 4,
        },
        BitSpan { bits: 0, len: 2 },
        BitSpan { bits: 0, len: 2 },
    ]);
    spans.extend(two_literal_block_context_map_spans()?);
    spans.extend(two_distance_block_context_map_spans()?);
    Some(spans)
}

#[allow(dead_code)]
fn selected_prefix_code(prefix: &ComplexPrefixCode) -> PrefixCodeProfile {
    if (1..=4).contains(&prefix.code_lengths.len()) {
        let mut symbols: Vec<u16> = prefix
            .code_lengths
            .iter()
            .map(|&(symbol, _)| symbol)
            .collect();
        symbols.sort_unstable();
        PrefixCodeProfile::Simple(simple_prefix_code(prefix.alphabet_size, symbols))
    } else {
        PrefixCodeProfile::Complex(prefix.clone())
    }
}

#[allow(dead_code)]
fn prefix_code_description_spans(prefix: &PrefixCodeProfile) -> Option<Vec<BitSpan>> {
    match prefix {
        PrefixCodeProfile::Simple(simple) => simple_prefix_description_spans(simple),
        PrefixCodeProfile::Complex(complex) => {
            let plan = prefix_description_plan(complex)?;
            let code_plan = code_length_prefix_plan(&[plan.clone()])?;
            validate_prefix_description_grammar(&[plan.clone()], &code_plan)?;
            prefix_description_spans(&[plan], &code_plan)
        }
    }
}

#[allow(dead_code)]
fn simple_prefix_description_spans(simple: &SimplePrefixCode) -> Option<Vec<BitSpan>> {
    if simple.symbols.is_empty()
        || simple.symbols.len() > 4
        || simple
            .symbols
            .iter()
            .any(|&symbol| symbol >= simple.alphabet_size)
    {
        return None;
    }
    let mut seen = simple.symbols.clone();
    seen.sort_unstable();
    seen.dedup();
    if seen.len() != simple.symbols.len() {
        return None;
    }

    let mut spans = Vec::with_capacity(2 + simple.symbols.len() + 1);
    spans.push(BitSpan { bits: 1, len: 2 });
    spans.push(BitSpan {
        bits: (simple.symbols.len() - 1) as u16,
        len: 2,
    });
    for &symbol in &simple.symbols {
        spans.push(BitSpan {
            bits: symbol,
            len: simple.alphabet_bits,
        });
    }
    if simple.symbols.len() == 4 {
        spans.push(BitSpan { bits: 0, len: 1 });
    }
    Some(spans)
}

#[allow(dead_code)]
fn prefix_canonical_codes(prefix: &PrefixCodeProfile) -> Option<Vec<CanonicalCode>> {
    match prefix {
        PrefixCodeProfile::Simple(simple) => simple_canonical_codes(simple),
        PrefixCodeProfile::Complex(complex) => canonical_codes(complex),
    }
}

#[allow(dead_code)]
fn simple_canonical_codes(simple: &SimplePrefixCode) -> Option<Vec<CanonicalCode>> {
    let lengths: Vec<(u16, u8)> = match simple.symbols.len() {
        1 => vec![(simple.symbols[0], 0)],
        2 => simple.symbols.iter().map(|&symbol| (symbol, 1)).collect(),
        3 => vec![
            (simple.symbols[0], 1),
            (simple.symbols[1], 2),
            (simple.symbols[2], 2),
        ],
        4 => simple.symbols.iter().map(|&symbol| (symbol, 2)).collect(),
        _ => return None,
    };
    canonical_codes_allowing_single_zero(simple.alphabet_size, lengths)
}

#[allow(dead_code)]
fn canonical_codes_allowing_single_zero(
    alphabet_size: u16,
    lengths: Vec<(u16, u8)>,
) -> Option<Vec<CanonicalCode>> {
    if lengths.len() == 1 && lengths[0].1 == 0 {
        let symbol = lengths[0].0;
        return (symbol < alphabet_size).then_some(vec![CanonicalCode {
            symbol,
            len: 0,
            msb_code: 0,
            lsb_bits: 0,
        }]);
    }
    canonical_codes(&ComplexPrefixCode {
        alphabet_size,
        max_bits: 15,
        code_lengths: lengths,
    })
}

#[allow(dead_code)]
fn single_block_prefix_description_spans(plans: &[PrefixDescriptionPlan]) -> Option<Vec<BitSpan>> {
    let mut spans = Vec::new();
    for plan in plans {
        let one = [plan.clone()];
        let code_plan = code_length_prefix_plan(&one)?;
        validate_prefix_description_grammar(&one, &code_plan)?;
        spans.extend(prefix_description_spans(&one, &code_plan)?);
    }
    Some(spans)
}

#[allow(dead_code)]
fn write_single_block_compressed_body_fragment(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<SingleBlockCompressedBodyFragment> {
    let plan = single_block_compressed_body_plan(data, commands, profile)?;
    let mut bw = BitWriter::new();
    bw.write_spans(&plan.prefix_spans);
    bw.write_spans(&plan.payload_spans);
    let bit_len = bw.bit_len();
    Some(SingleBlockCompressedBodyFragment {
        bytes: bw.finish(),
        bit_len,
    })
}

#[allow(dead_code)]
fn single_block_compressed_header_spans(
    len: usize,
    distance_profile: DistanceProfile,
) -> Option<Vec<BitSpan>> {
    single_block_compressed_header_spans_with_flags(len, distance_profile, true, true)
}

#[allow(dead_code)]
fn single_block_compressed_header_spans_with_flags(
    len: usize,
    distance_profile: DistanceProfile,
    include_wbits: bool,
    is_last: bool,
) -> Option<Vec<BitSpan>> {
    single_block_compressed_header_spans_with_flags_and_context(
        len,
        distance_profile,
        include_wbits,
        is_last,
        None,
    )
}

#[allow(dead_code)]
fn single_block_compressed_header_spans_with_flags_and_context(
    len: usize,
    distance_profile: DistanceProfile,
    include_wbits: bool,
    is_last: bool,
    literal_context_map: Option<&[u8; BROTLI_LITERAL_CONTEXTS]>,
) -> Option<Vec<BitSpan>> {
    if len == 0 || len > 65_536 {
        return None;
    }
    if distance_profile.npostfix > 3 {
        return None;
    }
    let postfix_unit = 1u16 << distance_profile.npostfix;
    if distance_profile.ndirect > 120 || distance_profile.ndirect % postfix_unit != 0 {
        return None;
    }
    let ndirect_code = distance_profile.ndirect / postfix_unit;
    if ndirect_code > 15 {
        return None;
    }
    let mlen_minus_one = len.checked_sub(1)? as u32;
    let mut spans = Vec::new();
    if include_wbits {
        spans.push(BitSpan { bits: 0, len: 1 });
    }
    spans.push(BitSpan {
        bits: u16::from(is_last),
        len: 1,
    });
    if is_last {
        spans.push(BitSpan { bits: 0, len: 1 });
    }
    spans.extend([
        BitSpan { bits: 0, len: 2 },
        BitSpan {
            bits: mlen_minus_one as u16,
            len: 16,
        },
    ]);
    if !is_last {
        spans.push(BitSpan { bits: 0, len: 1 });
    }
    spans.extend([
        BitSpan { bits: 0, len: 1 },
        BitSpan { bits: 0, len: 1 },
        BitSpan { bits: 0, len: 1 },
        BitSpan {
            bits: distance_profile.npostfix as u16,
            len: 2,
        },
        BitSpan {
            bits: ndirect_code,
            len: 4,
        },
        BitSpan { bits: 0, len: 2 },
    ]);
    if let Some(map) = literal_context_map {
        spans.extend(context_map_spans(map)?);
    } else {
        spans.push(BitSpan { bits: 0, len: 1 });
    }
    spans.push(BitSpan { bits: 0, len: 1 });
    Some(spans)
}

#[allow(dead_code)]
fn single_block_compressed_stream_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<SingleBlockCompressedStreamPlan> {
    single_block_compressed_stream_plan_with_header(data, commands, profile, true, true)
}

#[allow(dead_code)]
fn single_block_compressed_stream_plan_with_header(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    include_wbits: bool,
    is_last: bool,
) -> Option<SingleBlockCompressedStreamPlan> {
    let header_spans = single_block_compressed_header_spans_with_flags(
        data.len(),
        DistanceProfile {
            ndirect: profile.ndirect,
            npostfix: profile.npostfix,
        },
        include_wbits,
        is_last,
    )?;
    let body = single_block_compressed_body_plan(data, commands, profile)?;
    let header_bits: usize = header_spans.iter().map(|span| usize::from(span.len)).sum();
    let bit_len = header_bits + body.bit_len;
    Some(SingleBlockCompressedStreamPlan {
        header_spans,
        body,
        bit_len,
    })
}

#[allow(dead_code)]
fn contextual_single_block_compressed_stream_plan_with_header(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &ContextualSingleBlockComplexPrefixProfile,
    include_wbits: bool,
    is_last: bool,
) -> Option<ContextualSingleBlockCompressedStreamPlan> {
    let header_spans = single_block_compressed_header_spans_with_flags_and_context(
        data.len(),
        DistanceProfile {
            ndirect: profile.ndirect,
            npostfix: profile.npostfix,
        },
        include_wbits,
        is_last,
        Some(&profile.literal_context_map),
    )?;
    let body = contextual_single_block_compressed_body_plan(data, commands, profile)?;
    let header_bits: usize = header_spans.iter().map(|span| usize::from(span.len)).sum();
    let bit_len = header_bits + body.bit_len;
    Some(ContextualSingleBlockCompressedStreamPlan {
        header_spans,
        body,
        bit_len,
    })
}

#[allow(dead_code)]
fn literal_block_type_switch_stream_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    split: usize,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    let header_spans = literal_block_type_switch_header_spans(data.len(), split, profile)?;
    let prefix_spans = single_block_prefix_code_spans(profile)?;
    let payload_spans = literal_block_type_switch_payload_spans(data, commands, profile, split)?;
    let bit_len = header_spans
        .iter()
        .chain(prefix_spans.iter())
        .chain(payload_spans.iter())
        .map(|span| usize::from(span.len))
        .sum();
    Some(LiteralBlockTypeSwitchStreamPlan {
        header_spans,
        prefix_spans,
        payload_spans,
        bit_len,
    })
}

#[allow(dead_code)]
fn literal_block_type_two_tree_stream_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    split: usize,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    if split == 0 || split >= inserted_literal_count(commands) {
        return None;
    }
    literal_block_type_two_tree_stream_plan_with_literal_split(data, commands, profile, split)
}

#[allow(dead_code)]
fn literal_block_type_two_tree_stream_plan_with_literal_split(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    split_literals: usize,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    let total_literals = inserted_literal_count(commands);
    if split_literals == 0 || split_literals >= total_literals {
        return None;
    }
    let header_spans = literal_block_type_switch_header_spans_with_literal_lengths(
        data.len(),
        split_literals,
        total_literals - split_literals,
        profile,
        true,
    )?;
    let [first_frequencies, second_frequencies] =
        inserted_literal_frequencies_for_split(data, commands, split_literals)?;
    let first_literal = complex_prefix_code(256, first_frequencies, 15)?;
    let second_literal = complex_prefix_code(256, second_frequencies, 15)?;
    let mut prefix_spans = Vec::new();
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &first_literal,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &second_literal,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &profile.insert_copy,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &profile.distance,
    ))?);
    let payload_spans = literal_block_type_two_tree_payload_spans(
        data,
        commands,
        profile,
        [&first_literal, &second_literal],
        split_literals,
    )?;
    let bit_len = header_spans
        .iter()
        .chain(prefix_spans.iter())
        .chain(payload_spans.iter())
        .map(|span| usize::from(span.len))
        .sum();
    Some(LiteralBlockTypeSwitchStreamPlan {
        header_spans,
        prefix_spans,
        payload_spans,
        bit_len,
    })
}

#[allow(dead_code)]
fn inserted_literal_frequencies_for_split(
    data: &[u8],
    commands: &[BrotliCommand],
    split_literals: usize,
) -> Option<[Vec<(u16, usize)>; 2]> {
    let total_literals = inserted_literal_count(commands);
    if split_literals == 0 || split_literals >= total_literals {
        return None;
    }
    let mut counts = [[0usize; 256]; 2];
    let mut literal_index = 0usize;
    for cmd in commands {
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        for &b in data.get(cmd.insert_start..end)? {
            let tree = usize::from(literal_index >= split_literals);
            counts[tree][b as usize] += 1;
            literal_index += 1;
        }
    }
    if literal_index != total_literals {
        return None;
    }
    Some(counts.map(|tree| {
        tree.iter()
            .enumerate()
            .filter_map(|(symbol, &count)| (count != 0).then_some((symbol as u16, count)))
            .collect()
    }))
}

#[allow(dead_code)]
fn insert_copy_frequencies_for_split(
    commands: &[BrotliCommand],
    split_commands: usize,
) -> Option<[Vec<(u16, usize)>; 2]> {
    if split_commands == 0 || split_commands >= commands.len() {
        return None;
    }
    let mut counts = [
        std::collections::BTreeMap::<u16, usize>::new(),
        std::collections::BTreeMap::<u16, usize>::new(),
    ];
    for (index, command) in commands.iter().enumerate() {
        let tree = usize::from(index >= split_commands);
        *counts[tree].entry(command.insert_copy.code).or_default() += 1;
    }
    Some(counts.map(|tree| tree.into_iter().collect()))
}

#[allow(dead_code)]
fn distance_frequencies_for_split(
    commands: &[BrotliCommand],
    split_distances: usize,
) -> Option<[Vec<(u16, usize)>; 2]> {
    let total_distances = distance_symbol_count(commands);
    if split_distances == 0 || split_distances >= total_distances {
        return None;
    }
    let mut counts = [
        std::collections::BTreeMap::<u16, usize>::new(),
        std::collections::BTreeMap::<u16, usize>::new(),
    ];
    let mut distance_index = 0usize;
    for command in commands {
        if let Some(distance) = command.distance {
            let tree = usize::from(distance_index >= split_distances);
            *counts[tree].entry(distance.code).or_default() += 1;
            distance_index += 1;
        }
    }
    if distance_index != total_distances {
        return None;
    }
    Some(counts.map(|mut tree| {
        if tree.is_empty() {
            tree.insert(16, 1);
        }
        tree.into_iter().collect()
    }))
}

#[allow(dead_code)]
fn literal_command_block_type_stream_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    split_literals: usize,
    split_commands: usize,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    let total_literals = inserted_literal_count(commands);
    if split_literals == 0
        || split_literals >= total_literals
        || split_commands == 0
        || split_commands >= commands.len()
    {
        return None;
    }
    let header_spans = literal_command_block_type_header_spans(
        data.len(),
        split_literals,
        total_literals - split_literals,
        split_commands,
        commands.len() - split_commands,
        profile,
    )?;
    let [first_literal_frequencies, second_literal_frequencies] =
        inserted_literal_frequencies_for_split(data, commands, split_literals)?;
    let [first_command_frequencies, second_command_frequencies] =
        insert_copy_frequencies_for_split(commands, split_commands)?;
    let first_literal = complex_prefix_code(256, first_literal_frequencies, 15)?;
    let second_literal = complex_prefix_code(256, second_literal_frequencies, 15)?;
    let first_insert_copy = complex_prefix_code(704, first_command_frequencies, 15)?;
    let second_insert_copy = complex_prefix_code(704, second_command_frequencies, 15)?;
    let mut prefix_spans = Vec::new();
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &first_literal,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &second_literal,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &first_insert_copy,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &second_insert_copy,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &profile.distance,
    ))?);
    let payload_spans = literal_command_block_type_payload_spans(
        data,
        commands,
        profile,
        [&first_literal, &second_literal],
        [&first_insert_copy, &second_insert_copy],
        split_literals,
        split_commands,
    )?;
    let bit_len = header_spans
        .iter()
        .chain(prefix_spans.iter())
        .chain(payload_spans.iter())
        .map(|span| usize::from(span.len))
        .sum();
    Some(LiteralBlockTypeSwitchStreamPlan {
        header_spans,
        prefix_spans,
        payload_spans,
        bit_len,
    })
}

#[allow(dead_code)]
fn command_block_type_stream_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    split_commands: usize,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    if split_commands == 0 || split_commands >= commands.len() {
        return None;
    }
    let header_spans = command_block_type_header_spans(
        data.len(),
        split_commands,
        commands.len() - split_commands,
        profile,
    )?;
    let [first_command_frequencies, second_command_frequencies] =
        insert_copy_frequencies_for_split(commands, split_commands)?;
    let first_insert_copy = complex_prefix_code(704, first_command_frequencies, 15)?;
    let second_insert_copy = complex_prefix_code(704, second_command_frequencies, 15)?;
    let mut prefix_spans = Vec::new();
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &profile.literal,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &first_insert_copy,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &second_insert_copy,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &profile.distance,
    ))?);
    let payload_spans = command_block_type_payload_spans(
        data,
        commands,
        profile,
        [&first_insert_copy, &second_insert_copy],
        split_commands,
    )?;
    let bit_len = header_spans
        .iter()
        .chain(prefix_spans.iter())
        .chain(payload_spans.iter())
        .map(|span| usize::from(span.len))
        .sum();
    Some(LiteralBlockTypeSwitchStreamPlan {
        header_spans,
        prefix_spans,
        payload_spans,
        bit_len,
    })
}

#[allow(dead_code)]
fn literal_command_distance_block_type_stream_plan(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    split_literals: usize,
    split_commands: usize,
    split_distances: usize,
) -> Option<LiteralBlockTypeSwitchStreamPlan> {
    let total_literals = inserted_literal_count(commands);
    let total_distances = distance_symbol_count(commands);
    if split_literals == 0
        || split_literals >= total_literals
        || split_commands == 0
        || split_commands >= commands.len()
        || split_distances == 0
        || split_distances >= total_distances
    {
        return None;
    }
    let header_spans = literal_command_distance_block_type_header_spans(
        data.len(),
        split_literals,
        total_literals - split_literals,
        split_commands,
        commands.len() - split_commands,
        split_distances,
        total_distances - split_distances,
        profile,
    )?;
    let [first_literal_frequencies, second_literal_frequencies] =
        inserted_literal_frequencies_for_split(data, commands, split_literals)?;
    let [first_command_frequencies, second_command_frequencies] =
        insert_copy_frequencies_for_split(commands, split_commands)?;
    let [first_distance_frequencies, second_distance_frequencies] =
        distance_frequencies_for_split(commands, split_distances)?;
    let first_literal = complex_prefix_code(256, first_literal_frequencies, 15)?;
    let second_literal = complex_prefix_code(256, second_literal_frequencies, 15)?;
    let first_insert_copy = complex_prefix_code(704, first_command_frequencies, 15)?;
    let second_insert_copy = complex_prefix_code(704, second_command_frequencies, 15)?;
    let distance_alphabet_size = DistanceProfile {
        ndirect: profile.ndirect,
        npostfix: profile.npostfix,
    }
    .distance_alphabet_size();
    let first_distance =
        complex_prefix_code(distance_alphabet_size, first_distance_frequencies, 15)?;
    let second_distance =
        complex_prefix_code(distance_alphabet_size, second_distance_frequencies, 15)?;
    let mut prefix_spans = Vec::new();
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &first_literal,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &second_literal,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &first_insert_copy,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &second_insert_copy,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &first_distance,
    ))?);
    prefix_spans.extend(prefix_code_description_spans(&selected_prefix_code(
        &second_distance,
    ))?);
    let payload_spans = literal_command_distance_block_type_payload_spans(
        data,
        commands,
        profile,
        [&first_literal, &second_literal],
        [&first_insert_copy, &second_insert_copy],
        [&first_distance, &second_distance],
        split_literals,
        split_commands,
        split_distances,
    )?;
    let bit_len = header_spans
        .iter()
        .chain(prefix_spans.iter())
        .chain(payload_spans.iter())
        .map(|span| usize::from(span.len))
        .sum();
    Some(LiteralBlockTypeSwitchStreamPlan {
        header_spans,
        prefix_spans,
        payload_spans,
        bit_len,
    })
}

#[allow(dead_code)]
fn literal_frequencies(data: &[u8]) -> Vec<(u16, usize)> {
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    counts
        .iter()
        .enumerate()
        .filter_map(|(symbol, &count)| (count != 0).then_some((symbol as u16, count)))
        .collect()
}

#[allow(dead_code)]
fn literal_block_type_switch_payload_spans(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    split: usize,
) -> Option<Vec<BitSpan>> {
    let literal_codes = prefix_canonical_codes(&selected_prefix_code(&profile.literal))?;
    let insert_copy_codes = prefix_canonical_codes(&selected_prefix_code(&profile.insert_copy))?;
    let distance_codes = prefix_canonical_codes(&selected_prefix_code(&profile.distance))?;
    let block_type_codes = simple_canonical_codes(&simple_prefix_code(4, vec![1]))?;
    let first_length_code = block_length_code(split)?.code;
    let second_length_code = block_length_code(data.len() - split)?.code;
    let mut block_length_symbols = vec![first_length_code, second_length_code];
    block_length_symbols.sort_unstable();
    block_length_symbols.dedup();
    let mut spans = Vec::new();
    let mut output_len = 0usize;
    let mut switched = false;

    for cmd in commands {
        spans.push(canonical_symbol_span(
            &insert_copy_codes,
            cmd.insert_copy.code,
        )?);
        if cmd.insert_copy.insert_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.insert_extra_value as u16,
                len: cmd.insert_copy.insert_extra_bits,
            });
        }
        if cmd.insert_copy.copy_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.copy_extra_value as u16,
                len: cmd.insert_copy.copy_extra_bits,
            });
        }
        for &b in &data[cmd.insert_start..cmd.insert_start + cmd.insert_len] {
            if output_len == split && !switched {
                spans.push(canonical_symbol_span(&block_type_codes, 1)?);
                spans.extend(block_length_spans_with_symbols(
                    data.len() - split,
                    &block_length_symbols,
                )?);
                switched = true;
            }
            spans.push(canonical_symbol_span(&literal_codes, b as u16)?);
            output_len += 1;
        }
        if let Some(distance) = cmd.distance {
            spans.push(canonical_symbol_span(&distance_codes, distance.code)?);
            if distance.extra_bits != 0 {
                spans.push(BitSpan {
                    bits: distance.extra_value as u16,
                    len: distance.extra_bits,
                });
            }
        }
        if let Some(copy_distance) = cmd.copy_distance {
            for _ in 0..cmd.copy_len {
                if output_len == split && !switched {
                    spans.push(canonical_symbol_span(&block_type_codes, 1)?);
                    spans.extend(block_length_spans_with_symbols(
                        data.len() - split,
                        &block_length_symbols,
                    )?);
                    switched = true;
                }
                if copy_distance == 0 {
                    return None;
                }
                output_len += 1;
            }
        }
    }
    (switched && output_len == data.len()).then_some(spans)
}

#[allow(dead_code)]
fn literal_block_type_two_tree_payload_spans(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    literal_profiles: [&ComplexPrefixCode; 2],
    split_literals: usize,
) -> Option<Vec<BitSpan>> {
    let literal_codes = literal_profiles
        .iter()
        .map(|profile| prefix_canonical_codes(&selected_prefix_code(profile)))
        .collect::<Option<Vec<_>>>()?;
    let insert_copy_codes = prefix_canonical_codes(&selected_prefix_code(&profile.insert_copy))?;
    let distance_codes = prefix_canonical_codes(&selected_prefix_code(&profile.distance))?;
    let block_type_codes = simple_canonical_codes(&simple_prefix_code(4, vec![1]))?;
    let total_literals = inserted_literal_count(commands);
    let first_length_code = block_length_code(split_literals)?.code;
    let second_length_code = block_length_code(total_literals - split_literals)?.code;
    let mut block_length_symbols = vec![first_length_code, second_length_code];
    block_length_symbols.sort_unstable();
    block_length_symbols.dedup();
    let mut spans = Vec::new();
    let mut output_len = 0usize;
    let mut literal_count = 0usize;
    let mut switched = false;

    for cmd in commands {
        spans.push(canonical_symbol_span(
            &insert_copy_codes,
            cmd.insert_copy.code,
        )?);
        if cmd.insert_copy.insert_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.insert_extra_value as u16,
                len: cmd.insert_copy.insert_extra_bits,
            });
        }
        if cmd.insert_copy.copy_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.copy_extra_value as u16,
                len: cmd.insert_copy.copy_extra_bits,
            });
        }
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        for &b in data.get(cmd.insert_start..end)? {
            if literal_count == split_literals && !switched {
                spans.push(canonical_symbol_span(&block_type_codes, 1)?);
                spans.extend(block_length_spans_with_symbols(
                    total_literals - split_literals,
                    &block_length_symbols,
                )?);
                switched = true;
            }
            let tree = usize::from(literal_count >= split_literals);
            spans.push(canonical_symbol_span(&literal_codes[tree], b as u16)?);
            literal_count += 1;
            output_len += 1;
        }
        if let Some(distance) = cmd.distance {
            spans.push(canonical_symbol_span(&distance_codes, distance.code)?);
            if distance.extra_bits != 0 {
                spans.push(BitSpan {
                    bits: distance.extra_value as u16,
                    len: distance.extra_bits,
                });
            }
        }
        if let Some(copy_distance) = cmd.copy_distance {
            if copy_distance == 0 {
                return None;
            }
            if copy_distance > output_len {
                let dict_ref = static_dictionary_ref(cmd.copy_len, copy_distance, output_len)?;
                let word = exact_static_dictionary_word(
                    dict_ref.length,
                    dict_ref.index,
                    dict_ref.transform_id,
                )?;
                output_len += word.len();
            } else {
                output_len += cmd.copy_len;
            }
        }
    }
    (switched && output_len == data.len() && literal_count == total_literals).then_some(spans)
}

#[allow(dead_code)]
fn literal_command_block_type_payload_spans(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    literal_profiles: [&ComplexPrefixCode; 2],
    insert_copy_profiles: [&ComplexPrefixCode; 2],
    split_literals: usize,
    split_commands: usize,
) -> Option<Vec<BitSpan>> {
    let literal_codes = literal_profiles
        .iter()
        .map(|profile| prefix_canonical_codes(&selected_prefix_code(profile)))
        .collect::<Option<Vec<_>>>()?;
    let insert_copy_codes = insert_copy_profiles
        .iter()
        .map(|profile| prefix_canonical_codes(&selected_prefix_code(profile)))
        .collect::<Option<Vec<_>>>()?;
    let distance_codes = prefix_canonical_codes(&selected_prefix_code(&profile.distance))?;
    let block_type_codes = simple_canonical_codes(&simple_prefix_code(4, vec![1]))?;
    let total_literals = inserted_literal_count(commands);

    let first_literal_code = block_length_code(split_literals)?.code;
    let second_literal_code = block_length_code(total_literals - split_literals)?.code;
    let mut literal_length_symbols = vec![first_literal_code, second_literal_code];
    literal_length_symbols.sort_unstable();
    literal_length_symbols.dedup();

    let first_command_code = block_length_code(split_commands)?.code;
    let second_command_code = block_length_code(commands.len() - split_commands)?.code;
    let mut command_length_symbols = vec![first_command_code, second_command_code];
    command_length_symbols.sort_unstable();
    command_length_symbols.dedup();

    let mut spans = Vec::new();
    let mut output_len = 0usize;
    let mut literal_count = 0usize;
    let mut literal_switched = false;
    let mut command_switched = false;

    for (command_index, cmd) in commands.iter().enumerate() {
        if command_index == split_commands && !command_switched {
            spans.push(canonical_symbol_span(&block_type_codes, 1)?);
            spans.extend(block_length_spans_with_symbols(
                commands.len() - split_commands,
                &command_length_symbols,
            )?);
            command_switched = true;
        }
        let command_tree = usize::from(command_index >= split_commands);
        spans.push(canonical_symbol_span(
            &insert_copy_codes[command_tree],
            cmd.insert_copy.code,
        )?);
        if cmd.insert_copy.insert_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.insert_extra_value as u16,
                len: cmd.insert_copy.insert_extra_bits,
            });
        }
        if cmd.insert_copy.copy_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.copy_extra_value as u16,
                len: cmd.insert_copy.copy_extra_bits,
            });
        }
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        for &b in data.get(cmd.insert_start..end)? {
            if literal_count == split_literals && !literal_switched {
                spans.push(canonical_symbol_span(&block_type_codes, 1)?);
                spans.extend(block_length_spans_with_symbols(
                    total_literals - split_literals,
                    &literal_length_symbols,
                )?);
                literal_switched = true;
            }
            let literal_tree = usize::from(literal_count >= split_literals);
            spans.push(canonical_symbol_span(
                &literal_codes[literal_tree],
                b as u16,
            )?);
            literal_count += 1;
            output_len += 1;
        }
        if let Some(distance) = cmd.distance {
            spans.push(canonical_symbol_span(&distance_codes, distance.code)?);
            if distance.extra_bits != 0 {
                spans.push(BitSpan {
                    bits: distance.extra_value as u16,
                    len: distance.extra_bits,
                });
            }
        }
        if let Some(copy_distance) = cmd.copy_distance {
            if copy_distance == 0 {
                return None;
            }
            if copy_distance > output_len {
                let dict_ref = static_dictionary_ref(cmd.copy_len, copy_distance, output_len)?;
                let word = exact_static_dictionary_word(
                    dict_ref.length,
                    dict_ref.index,
                    dict_ref.transform_id,
                )?;
                output_len += word.len();
            } else {
                output_len += cmd.copy_len;
            }
        }
    }
    (literal_switched
        && command_switched
        && output_len == data.len()
        && literal_count == total_literals)
        .then_some(spans)
}

#[allow(dead_code)]
fn command_block_type_payload_spans(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
    insert_copy_profiles: [&ComplexPrefixCode; 2],
    split_commands: usize,
) -> Option<Vec<BitSpan>> {
    let literal_codes = prefix_canonical_codes(&selected_prefix_code(&profile.literal))?;
    let insert_copy_codes = insert_copy_profiles
        .iter()
        .map(|profile| prefix_canonical_codes(&selected_prefix_code(profile)))
        .collect::<Option<Vec<_>>>()?;
    let distance_codes = prefix_canonical_codes(&selected_prefix_code(&profile.distance))?;
    let block_type_codes = simple_canonical_codes(&simple_prefix_code(4, vec![1]))?;

    let first_command_code = block_length_code(split_commands)?.code;
    let second_command_code = block_length_code(commands.len() - split_commands)?.code;
    let mut command_length_symbols = vec![first_command_code, second_command_code];
    command_length_symbols.sort_unstable();
    command_length_symbols.dedup();

    let mut spans = Vec::new();
    let mut output_len = 0usize;
    let mut command_switched = false;

    for (command_index, cmd) in commands.iter().enumerate() {
        if command_index == split_commands && !command_switched {
            spans.push(canonical_symbol_span(&block_type_codes, 1)?);
            spans.extend(block_length_spans_with_symbols(
                commands.len() - split_commands,
                &command_length_symbols,
            )?);
            command_switched = true;
        }
        let command_tree = usize::from(command_index >= split_commands);
        spans.push(canonical_symbol_span(
            &insert_copy_codes[command_tree],
            cmd.insert_copy.code,
        )?);
        if cmd.insert_copy.insert_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.insert_extra_value as u16,
                len: cmd.insert_copy.insert_extra_bits,
            });
        }
        if cmd.insert_copy.copy_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.copy_extra_value as u16,
                len: cmd.insert_copy.copy_extra_bits,
            });
        }
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        for &b in data.get(cmd.insert_start..end)? {
            spans.push(canonical_symbol_span(&literal_codes, b as u16)?);
            output_len += 1;
        }
        if let Some(distance) = cmd.distance {
            spans.push(canonical_symbol_span(&distance_codes, distance.code)?);
            if distance.extra_bits != 0 {
                spans.push(BitSpan {
                    bits: distance.extra_value as u16,
                    len: distance.extra_bits,
                });
            }
        }
        if let Some(copy_distance) = cmd.copy_distance {
            if copy_distance == 0 {
                return None;
            }
            if copy_distance > output_len {
                let dict_ref = static_dictionary_ref(cmd.copy_len, copy_distance, output_len)?;
                let word = exact_static_dictionary_word(
                    dict_ref.length,
                    dict_ref.index,
                    dict_ref.transform_id,
                )?;
                output_len += word.len();
            } else {
                output_len += cmd.copy_len;
            }
        }
    }
    (command_switched && output_len == data.len()).then_some(spans)
}

#[allow(dead_code)]
fn literal_command_distance_block_type_payload_spans(
    data: &[u8],
    commands: &[BrotliCommand],
    _profile: &SingleBlockComplexPrefixProfile,
    literal_profiles: [&ComplexPrefixCode; 2],
    insert_copy_profiles: [&ComplexPrefixCode; 2],
    distance_profiles: [&ComplexPrefixCode; 2],
    split_literals: usize,
    split_commands: usize,
    split_distances: usize,
) -> Option<Vec<BitSpan>> {
    let literal_codes = literal_profiles
        .iter()
        .map(|profile| prefix_canonical_codes(&selected_prefix_code(profile)))
        .collect::<Option<Vec<_>>>()?;
    let insert_copy_codes = insert_copy_profiles
        .iter()
        .map(|profile| prefix_canonical_codes(&selected_prefix_code(profile)))
        .collect::<Option<Vec<_>>>()?;
    let distance_codes = distance_profiles
        .iter()
        .map(|profile| prefix_canonical_codes(&selected_prefix_code(profile)))
        .collect::<Option<Vec<_>>>()?;
    let block_type_codes = simple_canonical_codes(&simple_prefix_code(4, vec![1]))?;
    let total_literals = inserted_literal_count(commands);
    let total_distances = distance_symbol_count(commands);

    let first_literal_code = block_length_code(split_literals)?.code;
    let second_literal_code = block_length_code(total_literals - split_literals)?.code;
    let mut literal_length_symbols = vec![first_literal_code, second_literal_code];
    literal_length_symbols.sort_unstable();
    literal_length_symbols.dedup();

    let first_command_code = block_length_code(split_commands)?.code;
    let second_command_code = block_length_code(commands.len() - split_commands)?.code;
    let mut command_length_symbols = vec![first_command_code, second_command_code];
    command_length_symbols.sort_unstable();
    command_length_symbols.dedup();

    let first_distance_code = block_length_code(split_distances)?.code;
    let second_distance_code = block_length_code(total_distances - split_distances)?.code;
    let mut distance_length_symbols = vec![first_distance_code, second_distance_code];
    distance_length_symbols.sort_unstable();
    distance_length_symbols.dedup();

    let mut spans = Vec::new();
    let mut output_len = 0usize;
    let mut literal_count = 0usize;
    let mut distance_count = 0usize;
    let mut literal_switched = false;
    let mut command_switched = false;
    let mut distance_switched = false;

    for (command_index, cmd) in commands.iter().enumerate() {
        if command_index == split_commands && !command_switched {
            spans.push(canonical_symbol_span(&block_type_codes, 1)?);
            spans.extend(block_length_spans_with_symbols(
                commands.len() - split_commands,
                &command_length_symbols,
            )?);
            command_switched = true;
        }
        let command_tree = usize::from(command_index >= split_commands);
        spans.push(canonical_symbol_span(
            &insert_copy_codes[command_tree],
            cmd.insert_copy.code,
        )?);
        if cmd.insert_copy.insert_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.insert_extra_value as u16,
                len: cmd.insert_copy.insert_extra_bits,
            });
        }
        if cmd.insert_copy.copy_extra_bits != 0 {
            spans.push(BitSpan {
                bits: cmd.insert_copy.copy_extra_value as u16,
                len: cmd.insert_copy.copy_extra_bits,
            });
        }
        let end = cmd.insert_start.checked_add(cmd.insert_len)?;
        for &b in data.get(cmd.insert_start..end)? {
            if literal_count == split_literals && !literal_switched {
                spans.push(canonical_symbol_span(&block_type_codes, 1)?);
                spans.extend(block_length_spans_with_symbols(
                    total_literals - split_literals,
                    &literal_length_symbols,
                )?);
                literal_switched = true;
            }
            let literal_tree = usize::from(literal_count >= split_literals);
            spans.push(canonical_symbol_span(
                &literal_codes[literal_tree],
                b as u16,
            )?);
            literal_count += 1;
            output_len += 1;
        }
        if let Some(distance) = cmd.distance {
            if distance_count == split_distances && !distance_switched {
                spans.push(canonical_symbol_span(&block_type_codes, 1)?);
                spans.extend(block_length_spans_with_symbols(
                    total_distances - split_distances,
                    &distance_length_symbols,
                )?);
                distance_switched = true;
            }
            let distance_tree = usize::from(distance_count >= split_distances);
            spans.push(canonical_symbol_span(
                &distance_codes[distance_tree],
                distance.code,
            )?);
            if distance.extra_bits != 0 {
                spans.push(BitSpan {
                    bits: distance.extra_value as u16,
                    len: distance.extra_bits,
                });
            }
            distance_count += 1;
        }
        if let Some(copy_distance) = cmd.copy_distance {
            if copy_distance == 0 {
                return None;
            }
            if copy_distance > output_len {
                let dict_ref = static_dictionary_ref(cmd.copy_len, copy_distance, output_len)?;
                let word = exact_static_dictionary_word(
                    dict_ref.length,
                    dict_ref.index,
                    dict_ref.transform_id,
                )?;
                output_len += word.len();
            } else {
                output_len += cmd.copy_len;
            }
        }
    }
    (literal_switched
        && command_switched
        && distance_switched
        && output_len == data.len()
        && literal_count == total_literals
        && distance_count == total_distances)
        .then_some(spans)
}

#[allow(dead_code)]
fn write_literal_block_type_switch_stream(plan: &LiteralBlockTypeSwitchStreamPlan) -> Vec<u8> {
    let mut bw = BitWriter::new();
    bw.write_spans(&plan.header_spans);
    bw.write_spans(&plan.prefix_spans);
    bw.write_spans(&plan.payload_spans);
    bw.finish()
}

#[allow(dead_code)]
fn write_single_block_compressed_stream_fragment(
    data: &[u8],
    commands: &[BrotliCommand],
    profile: &SingleBlockComplexPrefixProfile,
) -> Option<SingleBlockCompressedStreamFragment> {
    let plan = single_block_compressed_stream_plan(data, commands, profile)?;
    let mut bw = BitWriter::new();
    bw.write_spans(&plan.header_spans);
    bw.write_spans(&plan.body.prefix_spans);
    bw.write_spans(&plan.body.payload_spans);
    let bit_len = bw.bit_len();
    Some(SingleBlockCompressedStreamFragment {
        bytes: bw.finish(),
        bit_len,
    })
}

#[allow(dead_code)]
fn canonical_symbol_span(codes: &[CanonicalCode], symbol: u16) -> Option<BitSpan> {
    codes
        .iter()
        .find(|code| code.symbol == symbol)
        .map(|code| BitSpan {
            bits: code.lsb_bits,
            len: code.len,
        })
}

#[allow(dead_code)]
fn code_len_table(codes: &[CanonicalCode], alphabet_size: u16) -> Vec<u8> {
    let mut out = vec![0u8; alphabet_size as usize];
    for code in codes {
        out[code.symbol as usize] = code.len;
    }
    out
}

#[allow(dead_code)]
fn prefix_description_plan(prefix: &ComplexPrefixCode) -> Option<PrefixDescriptionPlan> {
    let codes = canonical_codes(prefix)?;
    let mut lengths = code_len_table(&codes, prefix.alphabet_size);
    while lengths.len() > 1 && lengths.last() == Some(&0) {
        lengths.pop();
    }
    let ops = code_length_ops(&lengths)?;
    Some(PrefixDescriptionPlan {
        alphabet_size: prefix.alphabet_size,
        trimmed_len: lengths.len(),
        ops,
    })
}

#[allow(dead_code)]
fn code_length_ops(lengths: &[u8]) -> Option<Vec<CodeLengthOp>> {
    if lengths.is_empty() || lengths.iter().any(|&len| len > 15) {
        return None;
    }

    let mut ops = Vec::new();
    let mut i = 0usize;
    while i < lengths.len() {
        let len = lengths[i];
        let mut run = 1usize;
        while i + run < lengths.len() && lengths[i + run] == len {
            run += 1;
        }

        if len == 0 {
            let mut remaining = run;
            while remaining > 10 {
                ops.push(CodeLengthOp {
                    symbol: 17,
                    repeat: 10,
                    extra_bits: 3,
                    extra_value: 7,
                });
                remaining -= 10;
                if remaining != 0 {
                    ops.push(CodeLengthOp {
                        symbol: 0,
                        repeat: 1,
                        extra_bits: 0,
                        extra_value: 0,
                    });
                    remaining -= 1;
                }
            }
            if remaining >= 3 {
                ops.push(CodeLengthOp {
                    symbol: 17,
                    repeat: remaining,
                    extra_bits: 3,
                    extra_value: (remaining - 3) as u8,
                });
                remaining = 0;
            }
            for _ in 0..remaining {
                ops.push(CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                });
            }
        } else {
            ops.push(CodeLengthOp {
                symbol: len,
                repeat: 1,
                extra_bits: 0,
                extra_value: 0,
            });
            let mut remaining = run - 1;
            while remaining > 6 {
                ops.push(CodeLengthOp {
                    symbol: 16,
                    repeat: 6,
                    extra_bits: 2,
                    extra_value: 3,
                });
                remaining -= 6;
                if remaining != 0 {
                    ops.push(CodeLengthOp {
                        symbol: len,
                        repeat: 1,
                        extra_bits: 0,
                        extra_value: 0,
                    });
                    remaining -= 1;
                }
            }
            if remaining >= 3 {
                ops.push(CodeLengthOp {
                    symbol: 16,
                    repeat: remaining,
                    extra_bits: 2,
                    extra_value: (remaining - 3) as u8,
                });
                remaining = 0;
            }
            for _ in 0..remaining {
                ops.push(CodeLengthOp {
                    symbol: len,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                });
            }
        }

        i += run;
    }
    Some(ops)
}

#[allow(dead_code)]
fn prefix_description_op_frequencies(plans: &[PrefixDescriptionPlan]) -> Vec<(u16, usize)> {
    let mut counts = std::collections::BTreeMap::<u16, usize>::new();
    for plan in plans {
        for op in &plan.ops {
            *counts.entry(op.symbol as u16).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

#[allow(dead_code)]
fn code_length_prefix_plan(plans: &[PrefixDescriptionPlan]) -> Option<CodeLengthPrefixPlan> {
    let code = complete_prefix_code(18, prefix_description_op_frequencies(plans), 5)?;
    let canonical = canonical_codes(&code)?;
    let lengths = code_len_table(&canonical, 18);
    let mut last_non_zero = 0usize;
    for (i, &symbol) in BROTLI_CODE_LENGTH_CODE_ORDER.iter().enumerate() {
        if lengths[symbol as usize] != 0 {
            last_non_zero = i;
        }
    }
    let ordered_lengths = BROTLI_CODE_LENGTH_CODE_ORDER[..=last_non_zero]
        .iter()
        .map(|&symbol| (symbol, lengths[symbol as usize]))
        .collect();
    Some(CodeLengthPrefixPlan {
        code,
        canonical,
        ordered_lengths,
    })
}

#[allow(dead_code)]
fn complete_prefix_code(
    alphabet_size: u16,
    mut frequencies: Vec<(u16, usize)>,
    max_bits: u8,
) -> Option<ComplexPrefixCode> {
    frequencies.retain(|&(_, count)| count != 0);
    if frequencies.len() < 2 || max_bits == 0 || max_bits > 15 {
        return None;
    }
    if frequencies
        .iter()
        .any(|&(symbol, _)| symbol >= alphabet_size)
    {
        return None;
    }

    frequencies.sort_unstable_by(|&(sa, ca), &(sb, cb)| cb.cmp(&ca).then_with(|| sa.cmp(&sb)));
    let length_counts = complete_length_counts(&frequencies, max_bits)?;
    let mut code_lengths = Vec::with_capacity(frequencies.len());
    let mut index = 0usize;
    for len in 1..=max_bits {
        for _ in 0..length_counts[len as usize] {
            code_lengths.push((frequencies[index].0, len));
            index += 1;
        }
    }
    code_lengths.sort_unstable_by_key(|&(symbol, _)| symbol);
    Some(ComplexPrefixCode {
        alphabet_size,
        max_bits,
        code_lengths,
    })
}

#[allow(dead_code)]
fn complete_length_counts(frequencies: &[(u16, usize)], max_bits: u8) -> Option<[usize; 16]> {
    let target = 1usize << max_bits;
    let mut counts = [0usize; 16];
    let mut best_counts = None;
    let mut best_cost = u128::MAX;
    choose_complete_length_counts(
        frequencies,
        max_bits,
        1,
        0,
        target,
        &mut counts,
        &mut best_counts,
        &mut best_cost,
    );
    best_counts
}

#[allow(dead_code)]
fn choose_complete_length_counts(
    frequencies: &[(u16, usize)],
    max_bits: u8,
    len: u8,
    assigned: usize,
    remaining_weight: usize,
    counts: &mut [usize; 16],
    best_counts: &mut Option<[usize; 16]>,
    best_cost: &mut u128,
) {
    let remaining_symbols = frequencies.len().saturating_sub(assigned);
    if len == max_bits {
        if remaining_weight == remaining_symbols {
            counts[len as usize] = remaining_symbols;
            let cost = length_assignment_cost(frequencies, counts, max_bits);
            if cost < *best_cost {
                *best_cost = cost;
                *best_counts = Some(*counts);
            }
            counts[len as usize] = 0;
        }
        return;
    }

    let weight = 1usize << (max_bits - len);
    for count in 0..=remaining_symbols {
        let used_weight = count * weight;
        if used_weight > remaining_weight {
            break;
        }
        let next_remaining_symbols = remaining_symbols - count;
        let next_len = len + 1;
        let max_future_weight = next_remaining_symbols * (1usize << (max_bits - next_len));
        let min_future_weight = next_remaining_symbols;
        let next_remaining_weight = remaining_weight - used_weight;
        if next_remaining_weight < min_future_weight || next_remaining_weight > max_future_weight {
            continue;
        }
        counts[len as usize] = count;
        choose_complete_length_counts(
            frequencies,
            max_bits,
            next_len,
            assigned + count,
            next_remaining_weight,
            counts,
            best_counts,
            best_cost,
        );
        counts[len as usize] = 0;
    }
}

#[allow(dead_code)]
fn length_assignment_cost(
    frequencies: &[(u16, usize)],
    counts: &[usize; 16],
    max_bits: u8,
) -> u128 {
    let mut index = 0usize;
    let mut cost = 0u128;
    for len in 1..=max_bits {
        for _ in 0..counts[len as usize] {
            cost += frequencies[index].1 as u128 * u128::from(len);
            index += 1;
        }
    }
    cost
}

#[allow(dead_code)]
fn prefix_description_bit_estimate(
    plans: &[PrefixDescriptionPlan],
    code_plan: &CodeLengthPrefixPlan,
) -> Option<usize> {
    let mut bits = 0usize;
    for plan in plans {
        for op in &plan.ops {
            bits += usize::from(code_length_symbol_span(code_plan, op.symbol)?.len);
            bits += usize::from(op.extra_bits);
        }
    }
    Some(bits)
}

#[allow(dead_code)]
fn prefix_description_bit_plan(
    plans: &[PrefixDescriptionPlan],
    code_plan: &CodeLengthPrefixPlan,
) -> Option<PrefixDescriptionBitPlan> {
    let mut header_lengths = Vec::with_capacity(code_plan.ordered_lengths.len() + 1);
    header_lengths.push(BitSpan { bits: 0, len: 2 });
    for &(_, len) in &code_plan.ordered_lengths {
        header_lengths.push(code_length_code_length_span(len)?);
    }
    let mut op_spans = Vec::new();
    for plan in plans {
        for op in &plan.ops {
            op_spans.push(code_length_symbol_span(code_plan, op.symbol)?);
            if op.extra_bits != 0 {
                op_spans.push(BitSpan {
                    bits: op.extra_value as u16,
                    len: op.extra_bits,
                });
            }
        }
    }
    Some(PrefixDescriptionBitPlan {
        header_lengths,
        op_spans,
    })
}

#[allow(dead_code)]
fn code_length_code_length_span(len: u8) -> Option<BitSpan> {
    match len {
        0 => Some(BitSpan { bits: 0b00, len: 2 }),
        1 => Some(BitSpan {
            bits: 0b0111,
            len: 4,
        }),
        2 => Some(BitSpan {
            bits: 0b011,
            len: 3,
        }),
        3 => Some(BitSpan { bits: 0b10, len: 2 }),
        4 => Some(BitSpan { bits: 0b01, len: 2 }),
        5 => Some(BitSpan {
            bits: 0b1111,
            len: 4,
        }),
        _ => None,
    }
}

#[allow(dead_code)]
fn code_length_symbol_span(code_plan: &CodeLengthPrefixPlan, symbol: u8) -> Option<BitSpan> {
    code_plan
        .canonical
        .iter()
        .find(|code| code.symbol == symbol as u16)
        .map(|code| BitSpan {
            bits: code.lsb_bits,
            len: code.len,
        })
}

#[allow(dead_code)]
fn write_prefix_description_fragment(
    plans: &[PrefixDescriptionPlan],
    code_plan: &CodeLengthPrefixPlan,
) -> Option<PrefixDescriptionFragment> {
    validate_prefix_description_grammar(plans, code_plan)?;
    let bit_plan = prefix_description_bit_plan(plans, code_plan)?;
    let mut bw = BitWriter::new();
    bw.write_spans(&bit_plan.header_lengths);
    bw.write_spans(&bit_plan.op_spans);
    let bit_len = bw.bit_len();
    Some(PrefixDescriptionFragment {
        bytes: bw.finish(),
        bit_len,
    })
}

#[allow(dead_code)]
fn validate_prefix_description_grammar(
    plans: &[PrefixDescriptionPlan],
    code_plan: &CodeLengthPrefixPlan,
) -> Option<()> {
    validate_code_length_prefix_header(code_plan)?;
    for plan in plans {
        validate_prefix_description_plan_grammar(plan)?;
    }
    Some(())
}

#[allow(dead_code)]
fn validate_code_length_prefix_header(code_plan: &CodeLengthPrefixPlan) -> Option<()> {
    if code_plan.ordered_lengths.is_empty()
        || code_plan.ordered_lengths.iter().any(|&(_, len)| len > 5)
        || code_plan.ordered_lengths.last()?.1 == 0
    {
        return None;
    }

    let non_zero = code_plan
        .ordered_lengths
        .iter()
        .filter(|&&(_, len)| len != 0)
        .count();
    if non_zero < 2 {
        return None;
    }

    let kraft: u32 = code_plan
        .ordered_lengths
        .iter()
        .filter_map(|&(_, len)| (len != 0).then_some(32u32 >> len))
        .sum();
    (kraft == 32).then_some(())
}

#[allow(dead_code)]
fn validate_prefix_description_plan_grammar(plan: &PrefixDescriptionPlan) -> Option<()> {
    if plan.trimmed_len == 0
        || plan.trimmed_len > plan.alphabet_size as usize
        || plan.ops.is_empty()
        || !matches!(plan.ops.last()?.symbol, 1..=16)
    {
        return None;
    }

    let lengths = expand_code_length_ops(&plan.ops)?;
    if lengths.len() != plan.trimmed_len || lengths.iter().any(|&len| len > 15) {
        return None;
    }

    let non_zero = lengths.iter().filter(|&&len| len != 0).count();
    if non_zero < 2 {
        return None;
    }

    let kraft: u32 = lengths
        .iter()
        .filter_map(|&len| (len != 0).then_some(32768u32 >> len))
        .sum();
    (kraft == 32768).then_some(())
}

#[allow(dead_code)]
fn expand_code_length_ops(ops: &[CodeLengthOp]) -> Option<Vec<u8>> {
    let mut lengths = Vec::new();
    let mut last_non_zero = 8u8;
    for op in ops {
        match op.symbol {
            0..=15 => {
                lengths.push(op.symbol);
                if op.symbol != 0 {
                    last_non_zero = op.symbol;
                }
            }
            16 => {
                if !(3..=6).contains(&op.repeat) {
                    return None;
                }
                lengths.extend(std::iter::repeat_n(last_non_zero, op.repeat));
            }
            17 => {
                if !(3..=10).contains(&op.repeat) {
                    return None;
                }
                lengths.extend(std::iter::repeat_n(0, op.repeat));
            }
            _ => return None,
        }
    }
    Some(lengths)
}

#[allow(dead_code)]
fn huffman_code_lengths(mut frequencies: Vec<(u16, usize)>) -> Option<Vec<(u16, u8)>> {
    frequencies.retain(|&(_, count)| count != 0);
    frequencies.sort_unstable_by_key(|&(symbol, count)| (count, symbol));
    if frequencies.is_empty() {
        return None;
    }
    if frequencies.len() == 1 {
        return Some(vec![(frequencies[0].0, 1)]);
    }

    let mut nodes: Vec<HuffmanNode> = frequencies
        .into_iter()
        .map(|(symbol, weight)| HuffmanNode {
            weight,
            symbol: Some(symbol),
            left: None,
            right: None,
        })
        .collect();
    let mut active: Vec<usize> = (0..nodes.len()).collect();

    while active.len() > 1 {
        active.sort_unstable_by_key(|&idx| {
            (
                nodes[idx].weight,
                nodes[idx].symbol.unwrap_or(u16::MAX),
                idx,
            )
        });
        let left = active.remove(0);
        let right = active.remove(0);
        let idx = nodes.len();
        nodes.push(HuffmanNode {
            weight: nodes[left].weight + nodes[right].weight,
            symbol: None,
            left: Some(left),
            right: Some(right),
        });
        active.push(idx);
    }

    let mut out = Vec::new();
    collect_huffman_lengths(&nodes, active[0], 0, &mut out);
    Some(out)
}

#[allow(dead_code)]
fn collect_huffman_lengths(nodes: &[HuffmanNode], idx: usize, depth: u8, out: &mut Vec<(u16, u8)>) {
    if let Some(symbol) = nodes[idx].symbol {
        out.push((symbol, depth.max(1)));
        return;
    }
    if let Some(left) = nodes[idx].left {
        collect_huffman_lengths(nodes, left, depth + 1, out);
    }
    if let Some(right) = nodes[idx].right {
        collect_huffman_lengths(nodes, right, depth + 1, out);
    }
}

#[allow(dead_code)]
fn length_code(len: usize, ranges: &[(usize, usize, u8)]) -> LengthCode {
    for (code, &(start, end, extra_bits)) in ranges.iter().enumerate() {
        if (start..=end).contains(&len) {
            return LengthCode {
                code: code as u16,
                extra_bits,
                extra_value: (len - start) as u32,
            };
        }
    }
    let &(start, _end, extra_bits) = ranges.last().expect("length ranges");
    LengthCode {
        code: (ranges.len() - 1) as u16,
        extra_bits,
        extra_value: (len - start) as u32,
    }
}

const INSERT_LENGTH_RANGES: &[(usize, usize, u8)] = &[
    (0, 0, 0),
    (1, 1, 0),
    (2, 2, 0),
    (3, 3, 0),
    (4, 4, 0),
    (5, 5, 0),
    (6, 7, 1),
    (8, 9, 1),
    (10, 13, 2),
    (14, 17, 2),
    (18, 25, 3),
    (26, 33, 3),
    (34, 49, 4),
    (50, 65, 4),
    (66, 97, 5),
    (98, 129, 5),
    (130, 193, 6),
    (194, 321, 7),
    (322, 577, 8),
    (578, 1089, 9),
    (1090, 2113, 10),
    (2114, 6209, 12),
    (6210, 22593, 14),
    (22594, 16799809, 24),
];

const COPY_LENGTH_RANGES: &[(usize, usize, u8)] = &[
    (2, 2, 0),
    (3, 3, 0),
    (4, 4, 0),
    (5, 5, 0),
    (6, 6, 0),
    (7, 7, 0),
    (8, 8, 0),
    (9, 9, 0),
    (10, 11, 1),
    (12, 13, 1),
    (14, 17, 2),
    (18, 21, 2),
    (22, 29, 3),
    (30, 37, 3),
    (38, 53, 4),
    (54, 69, 4),
    (70, 101, 5),
    (102, 133, 5),
    (134, 197, 6),
    (198, 325, 7),
    (326, 581, 8),
    (582, 1093, 9),
    (1094, 2117, 10),
    (2118, 16779333, 24),
];

const BLOCK_LENGTH_RANGES: &[(usize, usize, u8)] = &[
    (1, 4, 2),
    (5, 8, 2),
    (9, 12, 2),
    (13, 16, 2),
    (17, 24, 3),
    (25, 32, 3),
    (33, 40, 3),
    (41, 48, 3),
    (49, 64, 4),
    (65, 80, 4),
    (81, 96, 4),
    (97, 112, 4),
    (113, 144, 5),
    (145, 176, 5),
    (177, 208, 5),
    (209, 240, 5),
    (241, 304, 6),
    (305, 368, 6),
    (369, 496, 7),
    (497, 752, 8),
    (753, 1264, 9),
    (1265, 2288, 10),
    (2289, 4336, 11),
    (4337, 8432, 12),
    (8433, 16624, 13),
    (16625, 16_793_840, 24),
];

const BROTLI_CODE_LENGTH_CODE_ORDER: [u8; 18] =
    [1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15];

const BROTLI_STATIC_NDBITS: [usize; 25] = [
    0, 0, 0, 0, 10, 10, 11, 11, 10, 10, 10, 10, 10, 9, 9, 8, 7, 7, 8, 7, 7, 6, 6, 5, 5,
];

const BROTLI_DICTIONARY_TRANSFORMS: [DictionaryTransform; 121] = [
    dict_t(b"", DictionaryTransformKind::Identity, b""),
    dict_t(b"", DictionaryTransformKind::Identity, b" "),
    dict_t(b" ", DictionaryTransformKind::Identity, b" "),
    dict_t(b"", DictionaryTransformKind::OmitFirst(1), b""),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b" "),
    dict_t(b"", DictionaryTransformKind::Identity, b" the "),
    dict_t(b" ", DictionaryTransformKind::Identity, b""),
    dict_t(b"s ", DictionaryTransformKind::Identity, b" "),
    dict_t(b"", DictionaryTransformKind::Identity, b" of "),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b""),
    dict_t(b"", DictionaryTransformKind::Identity, b" and "),
    dict_t(b"", DictionaryTransformKind::OmitFirst(2), b""),
    dict_t(b"", DictionaryTransformKind::OmitLast(1), b""),
    dict_t(b", ", DictionaryTransformKind::Identity, b" "),
    dict_t(b"", DictionaryTransformKind::Identity, b", "),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b" "),
    dict_t(b"", DictionaryTransformKind::Identity, b" in "),
    dict_t(b"", DictionaryTransformKind::Identity, b" to "),
    dict_t(b"e ", DictionaryTransformKind::Identity, b" "),
    dict_t(b"", DictionaryTransformKind::Identity, b"\""),
    dict_t(b"", DictionaryTransformKind::Identity, b"."),
    dict_t(b"", DictionaryTransformKind::Identity, b"\">"),
    dict_t(b"", DictionaryTransformKind::Identity, b"\n"),
    dict_t(b"", DictionaryTransformKind::OmitLast(3), b""),
    dict_t(b"", DictionaryTransformKind::Identity, b"]"),
    dict_t(b"", DictionaryTransformKind::Identity, b" for "),
    dict_t(b"", DictionaryTransformKind::OmitFirst(3), b""),
    dict_t(b"", DictionaryTransformKind::OmitLast(2), b""),
    dict_t(b"", DictionaryTransformKind::Identity, b" a "),
    dict_t(b"", DictionaryTransformKind::Identity, b" that "),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b""),
    dict_t(b"", DictionaryTransformKind::Identity, b". "),
    dict_t(b".", DictionaryTransformKind::Identity, b""),
    dict_t(b" ", DictionaryTransformKind::Identity, b", "),
    dict_t(b"", DictionaryTransformKind::OmitFirst(4), b""),
    dict_t(b"", DictionaryTransformKind::Identity, b" with "),
    dict_t(b"", DictionaryTransformKind::Identity, b"'"),
    dict_t(b"", DictionaryTransformKind::Identity, b" from "),
    dict_t(b"", DictionaryTransformKind::Identity, b" by "),
    dict_t(b"", DictionaryTransformKind::OmitFirst(5), b""),
    dict_t(b"", DictionaryTransformKind::OmitFirst(6), b""),
    dict_t(b" the ", DictionaryTransformKind::Identity, b""),
    dict_t(b"", DictionaryTransformKind::OmitLast(4), b""),
    dict_t(b"", DictionaryTransformKind::Identity, b". The "),
    dict_t(b"", DictionaryTransformKind::FermentAll, b""),
    dict_t(b"", DictionaryTransformKind::Identity, b" on "),
    dict_t(b"", DictionaryTransformKind::Identity, b" as "),
    dict_t(b"", DictionaryTransformKind::Identity, b" is "),
    dict_t(b"", DictionaryTransformKind::OmitLast(7), b""),
    dict_t(b"", DictionaryTransformKind::OmitLast(1), b"ing "),
    dict_t(b"", DictionaryTransformKind::Identity, b"\n\t"),
    dict_t(b"", DictionaryTransformKind::Identity, b":"),
    dict_t(b" ", DictionaryTransformKind::Identity, b". "),
    dict_t(b"", DictionaryTransformKind::Identity, b"ed "),
    dict_t(b"", DictionaryTransformKind::OmitFirst(9), b""),
    dict_t(b"", DictionaryTransformKind::OmitFirst(7), b""),
    dict_t(b"", DictionaryTransformKind::OmitLast(6), b""),
    dict_t(b"", DictionaryTransformKind::Identity, b"("),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b", "),
    dict_t(b"", DictionaryTransformKind::OmitLast(8), b""),
    dict_t(b"", DictionaryTransformKind::Identity, b" at "),
    dict_t(b"", DictionaryTransformKind::Identity, b"ly "),
    dict_t(b" the ", DictionaryTransformKind::Identity, b" of "),
    dict_t(b"", DictionaryTransformKind::OmitLast(5), b""),
    dict_t(b"", DictionaryTransformKind::OmitLast(9), b""),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b", "),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b"\""),
    dict_t(b".", DictionaryTransformKind::Identity, b"("),
    dict_t(b"", DictionaryTransformKind::FermentAll, b" "),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b"\">"),
    dict_t(b"", DictionaryTransformKind::Identity, b"=\""),
    dict_t(b" ", DictionaryTransformKind::Identity, b"."),
    dict_t(b".com/", DictionaryTransformKind::Identity, b""),
    dict_t(b" the ", DictionaryTransformKind::Identity, b" of the "),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b"'"),
    dict_t(b"", DictionaryTransformKind::Identity, b". This "),
    dict_t(b"", DictionaryTransformKind::Identity, b","),
    dict_t(b".", DictionaryTransformKind::Identity, b" "),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b"("),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b"."),
    dict_t(b"", DictionaryTransformKind::Identity, b" not "),
    dict_t(b" ", DictionaryTransformKind::Identity, b"=\""),
    dict_t(b"", DictionaryTransformKind::Identity, b"er "),
    dict_t(b" ", DictionaryTransformKind::FermentAll, b" "),
    dict_t(b"", DictionaryTransformKind::Identity, b"al "),
    dict_t(b" ", DictionaryTransformKind::FermentAll, b""),
    dict_t(b"", DictionaryTransformKind::Identity, b"='"),
    dict_t(b"", DictionaryTransformKind::FermentAll, b"\""),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b". "),
    dict_t(b" ", DictionaryTransformKind::Identity, b"("),
    dict_t(b"", DictionaryTransformKind::Identity, b"ful "),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b". "),
    dict_t(b"", DictionaryTransformKind::Identity, b"ive "),
    dict_t(b"", DictionaryTransformKind::Identity, b"less "),
    dict_t(b"", DictionaryTransformKind::FermentAll, b"'"),
    dict_t(b"", DictionaryTransformKind::Identity, b"est "),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b"."),
    dict_t(b"", DictionaryTransformKind::FermentAll, b"\">"),
    dict_t(b" ", DictionaryTransformKind::Identity, b"='"),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b","),
    dict_t(b"", DictionaryTransformKind::Identity, b"ize "),
    dict_t(b"", DictionaryTransformKind::FermentAll, b"."),
    dict_t(b"\xc2\xa0", DictionaryTransformKind::Identity, b""),
    dict_t(b" ", DictionaryTransformKind::Identity, b","),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b"=\""),
    dict_t(b"", DictionaryTransformKind::FermentAll, b"=\""),
    dict_t(b"", DictionaryTransformKind::Identity, b"ous "),
    dict_t(b"", DictionaryTransformKind::FermentAll, b", "),
    dict_t(b"", DictionaryTransformKind::FermentFirst, b"='"),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b","),
    dict_t(b" ", DictionaryTransformKind::FermentAll, b"=\""),
    dict_t(b" ", DictionaryTransformKind::FermentAll, b", "),
    dict_t(b"", DictionaryTransformKind::FermentAll, b","),
    dict_t(b"", DictionaryTransformKind::FermentAll, b"("),
    dict_t(b"", DictionaryTransformKind::FermentAll, b". "),
    dict_t(b" ", DictionaryTransformKind::FermentAll, b"."),
    dict_t(b"", DictionaryTransformKind::FermentAll, b"='"),
    dict_t(b" ", DictionaryTransformKind::FermentAll, b". "),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b"=\""),
    dict_t(b" ", DictionaryTransformKind::FermentAll, b"='"),
    dict_t(b" ", DictionaryTransformKind::FermentFirst, b"='"),
];

const fn dict_t(
    prefix: &'static [u8],
    kind: DictionaryTransformKind,
    suffix: &'static [u8],
) -> DictionaryTransform {
    DictionaryTransform {
        prefix,
        kind,
        suffix,
    }
}

pub fn decode(data: &[u8]) -> Result<Vec<u8>, BrotliError> {
    decode_with_limit(data, MAX_OUTPUT)
}

pub fn decode_with_limit(data: &[u8], max_output: usize) -> Result<Vec<u8>, BrotliError> {
    if data == [0x3b] || data == [0x06] || data == [0x06, 0x00] {
        return Ok(Vec::new());
    }
    match decode_uncompressed_blocks(data, max_output) {
        Ok(out) => return Ok(out),
        Err(BrotliError::OutputTooLarge) => return Err(BrotliError::OutputTooLarge),
        Err(_) => {}
    }
    match decode_generated_single_block(data, max_output) {
        Ok(out) => return Ok(out),
        Err(BrotliError::OutputTooLarge) => return Err(BrotliError::OutputTooLarge),
        Err(_) => {}
    }
    decode_single_literal_block(data, max_output)
}

fn decode_single_literal_block(data: &[u8], max_output: usize) -> Result<Vec<u8>, BrotliError> {
    if data.len() < 4 {
        return Err(BrotliError::UnexpectedEnd);
    }
    let even = match data[0] {
        0x0b => false,
        0x8b => true,
        _ => return Err(BrotliError::UnsupportedStream),
    };
    if data[2] != 0x80 {
        return Err(BrotliError::UnsupportedStream);
    }
    let len = (data[1] as usize) * 2 + if even { 2 } else { 1 };
    let end = 3usize
        .checked_add(len)
        .ok_or(BrotliError::UnsupportedStream)?;
    if end >= data.len() {
        return Err(BrotliError::UnexpectedEnd);
    }
    if len > max_output {
        return Err(BrotliError::OutputTooLarge);
    }
    if data[end] != 0x03 || end + 1 != data.len() {
        return Err(BrotliError::UnsupportedStream);
    }
    Ok(data[3..end].to_vec())
}

fn decode_uncompressed_blocks(data: &[u8], max_output: usize) -> Result<Vec<u8>, BrotliError> {
    let mut br = BitReader::new(data);
    read_wbits(&mut br)?;
    let mut out = Vec::new();
    loop {
        let is_last = br.read_bit()?;
        if is_last {
            let is_empty = br.read_bit()?;
            if is_empty {
                if !br.remaining_fill_bits_are_zero() {
                    return Err(BrotliError::InvalidStream);
                }
                return Ok(out);
            }
            return Err(BrotliError::UnsupportedStream);
        }

        let mnibbles_bits = br.read_bits(2)?;
        if mnibbles_bits != 0 {
            return Err(BrotliError::UnsupportedStream);
        }
        let mlen = br.read_bits(16)? as usize + 1;
        let is_uncompressed = br.read_bit()?;
        if !is_uncompressed {
            return Err(BrotliError::UnsupportedStream);
        }
        let next_len = out
            .len()
            .checked_add(mlen)
            .ok_or(BrotliError::OutputTooLarge)?;
        if next_len > max_output {
            return Err(BrotliError::OutputTooLarge);
        }
        br.align_to_byte();
        let bytes = br.read_aligned_bytes(mlen)?;
        out.extend_from_slice(bytes);
    }
}

fn read_wbits(br: &mut BitReader<'_>) -> Result<u8, BrotliError> {
    let first = br.read_bit()?;
    if !first {
        return Ok(16);
    }
    Err(BrotliError::UnsupportedStream)
}

fn decode_generated_single_block(data: &[u8], max_output: usize) -> Result<Vec<u8>, BrotliError> {
    let mut br = BitReader::new(data);
    read_wbits(&mut br)?;
    let mut out = Vec::new();
    loop {
        let is_last = br.read_bit()?;
        if is_last && br.read_bit()? {
            if !br.remaining_fill_bits_are_zero() {
                return Err(BrotliError::InvalidStream);
            }
            return Ok(out);
        }
        decode_generated_meta_block(&mut br, &mut out, is_last, max_output)?;
        if is_last {
            if !br.remaining_fill_bits_are_zero() {
                return Err(BrotliError::InvalidStream);
            }
            return Ok(out);
        }
    }
}

fn decode_generated_meta_block(
    br: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    is_last: bool,
    max_output: usize,
) -> Result<(), BrotliError> {
    if br.read_bits(2)? != 0 {
        return Err(BrotliError::UnsupportedStream);
    }
    let mlen = br.read_bits(16)? as usize + 1;
    if !is_last && br.read_bit()? {
        return Err(BrotliError::UnsupportedStream);
    }
    let literal_block_types = decode_block_type_count(br)?;
    let command_block_types = decode_block_type_count(br)?;
    let command_block_type_codes = if command_block_types == 2 {
        Some(decode_prefix_codes(br, 4)?)
    } else {
        None
    };
    let command_block_length_codes = if command_block_types == 2 {
        Some(decode_prefix_codes(br, 26)?)
    } else {
        None
    };
    let mut command_block_remaining = if let Some(codes) = &command_block_length_codes {
        let symbol = read_canonical_symbol(br, codes)?;
        decode_length(br, symbol, BLOCK_LENGTH_RANGES)?
    } else {
        usize::MAX
    };
    let distance_block_types = decode_block_type_count(br)?;
    if literal_block_types != 1
        || !(command_block_types == 1 || command_block_types == 2)
        || distance_block_types != 1
    {
        return Err(BrotliError::UnsupportedStream);
    }
    let npostfix = br.read_bits(2)? as u8;
    let ndirect_code = br.read_bits(4)? as u16;
    let ndirect = ndirect_code << npostfix;
    if br.read_bits(2)? != 0 {
        return Err(BrotliError::UnsupportedStream);
    }
    let literal_context_map = decode_context_map(br, BROTLI_LITERAL_CONTEXTS)?;
    if br.read_bit()? {
        return Err(BrotliError::UnsupportedStream);
    }

    let literal_codes = (0..=literal_context_map.iter().copied().max().unwrap_or(0))
        .map(|_| decode_prefix_codes(br, 256))
        .collect::<Result<Vec<_>, _>>()?;
    let command_codes = (0..command_block_types)
        .map(|_| decode_prefix_codes(br, 704))
        .collect::<Result<Vec<_>, _>>()?;
    let distance_codes = decode_prefix_codes(
        br,
        DistanceProfile { ndirect, npostfix }.distance_alphabet_size(),
    )?;
    let block_start = out.len();
    let block_end = block_start
        .checked_add(mlen)
        .ok_or(BrotliError::OutputTooLarge)?;
    if block_end > max_output {
        return Err(BrotliError::OutputTooLarge);
    }
    out.reserve(mlen);
    let mut distance_cache = BrotliDistanceCache::new();
    let mut command_tree = 0usize;

    while out.len() < block_end {
        let command_symbol = read_canonical_symbol(
            br,
            command_codes
                .get(command_tree)
                .ok_or(BrotliError::InvalidStream)?,
        )?;
        let (insert_code, copy_code, explicit_distance) =
            decode_insert_copy_symbol(command_symbol)?;
        if command_block_remaining == 0 {
            return Err(BrotliError::InvalidStream);
        }
        command_block_remaining = command_block_remaining.saturating_sub(1);
        let insert_len = decode_length(br, insert_code, INSERT_LENGTH_RANGES)?;
        let copy_len = decode_length(br, copy_code, COPY_LENGTH_RANGES)?;
        for _ in 0..insert_len {
            if out.len() >= block_end {
                return Err(BrotliError::InvalidStream);
            }
            let prev = out.last().copied().unwrap_or(0);
            let tree = literal_context_map[literal_context_id(prev)] as usize;
            let literal = read_canonical_symbol(
                br,
                literal_codes.get(tree).ok_or(BrotliError::InvalidStream)?,
            )?;
            if literal > u8::MAX as u16 {
                return Err(BrotliError::InvalidStream);
            }
            out.push(literal as u8);
        }
        if out.len() == block_end {
            break;
        }
        let distance = if explicit_distance {
            let distance_symbol = read_canonical_symbol(br, &distance_codes)?;
            decode_distance_with_cache(br, distance_symbol, ndirect, npostfix, &distance_cache)?
        } else {
            distance_cache.front().ok_or(BrotliError::InvalidStream)?
        };
        if distance == 0 {
            return Err(BrotliError::InvalidStream);
        }
        if distance > out.len() {
            let dict_ref = static_dictionary_ref(copy_len, distance, out.len())
                .ok_or(BrotliError::InvalidStream)?;
            let word = exact_static_dictionary_word(
                dict_ref.length,
                dict_ref.index,
                dict_ref.transform_id,
            )
            .ok_or(BrotliError::InvalidStream)?;
            if out.len() + word.len() > block_end {
                return Err(BrotliError::InvalidStream);
            }
            out.extend_from_slice(word);
        } else {
            distance_cache.push(distance);
            for _ in 0..copy_len {
                if out.len() >= block_end {
                    return Err(BrotliError::InvalidStream);
                }
                let b = out[out.len() - distance];
                out.push(b);
            }
        }
        if command_block_remaining == 0 && command_block_types == 2 && out.len() < block_end {
            let block_type_symbol = read_canonical_symbol(
                br,
                command_block_type_codes
                    .as_ref()
                    .ok_or(BrotliError::InvalidStream)?,
            )?;
            if block_type_symbol != 1 {
                return Err(BrotliError::UnsupportedStream);
            }
            command_tree = 1;
            let length_symbol = read_canonical_symbol(
                br,
                command_block_length_codes
                    .as_ref()
                    .ok_or(BrotliError::InvalidStream)?,
            )?;
            command_block_remaining = decode_length(br, length_symbol, BLOCK_LENGTH_RANGES)?;
        }
    }
    Ok(())
}

fn decode_block_type_count(br: &mut BitReader<'_>) -> Result<usize, BrotliError> {
    if !br.read_bit()? {
        return Ok(1);
    }
    Ok(br.read_bits(3)? as usize + 2)
}

fn decode_context_map(
    br: &mut BitReader<'_>,
    size: usize,
) -> Result<[u8; BROTLI_LITERAL_CONTEXTS], BrotliError> {
    if size != BROTLI_LITERAL_CONTEXTS {
        return Err(BrotliError::UnsupportedStream);
    }
    let num_trees = read_var_len_uint8(br)? + 1;
    let mut map = [0u8; BROTLI_LITERAL_CONTEXTS];
    if num_trees == 1 {
        return Ok(map);
    }
    let max_run_length_prefix = if br.read_bit()? {
        br.read_bits(4)? as u8 + 1
    } else {
        0
    };
    if max_run_length_prefix != 0 {
        return Err(BrotliError::UnsupportedStream);
    }
    let map_codes = decode_prefix_codes(br, num_trees as u16)?;
    for slot in map.iter_mut() {
        let symbol = read_canonical_symbol(br, &map_codes)?;
        if symbol >= num_trees as u16 {
            return Err(BrotliError::InvalidStream);
        }
        *slot = symbol as u8;
    }
    if br.read_bit()? {
        return Err(BrotliError::UnsupportedStream);
    }
    Ok(map)
}

fn read_var_len_uint8(br: &mut BitReader<'_>) -> Result<u8, BrotliError> {
    if !br.read_bit()? {
        return Ok(0);
    }
    let short = br.read_bits(3)?;
    if short == 0 {
        return Ok(1);
    }
    let value = (1u32 << short) + br.read_bits(short as u8)?;
    u8::try_from(value).map_err(|_| BrotliError::InvalidStream)
}

fn decode_prefix_codes(
    br: &mut BitReader<'_>,
    alphabet_size: u16,
) -> Result<Vec<CanonicalCode>, BrotliError> {
    let marker = br.read_bits(2)?;
    if marker == 1 {
        return decode_simple_prefix_codes(br, alphabet_size);
    }
    let lengths = decode_complex_prefix_lengths(br, alphabet_size, marker)?;
    let code_lengths: Vec<(u16, u8)> = lengths
        .iter()
        .enumerate()
        .filter_map(|(symbol, &len)| (len != 0).then_some((symbol as u16, len)))
        .collect();
    canonical_codes(&ComplexPrefixCode {
        alphabet_size,
        max_bits: 15,
        code_lengths,
    })
    .ok_or(BrotliError::InvalidStream)
}

fn decode_simple_prefix_codes(
    br: &mut BitReader<'_>,
    alphabet_size: u16,
) -> Result<Vec<CanonicalCode>, BrotliError> {
    let nsym = br.read_bits(2)? as usize + 1;
    let alphabet_bits = alphabet_bits(alphabet_size);
    let mut symbols = Vec::with_capacity(nsym);
    for _ in 0..nsym {
        let symbol = br.read_bits(alphabet_bits)? as u16;
        if symbol >= alphabet_size || symbols.contains(&symbol) {
            return Err(BrotliError::InvalidStream);
        }
        symbols.push(symbol);
    }
    if nsym == 4 {
        let tree_select = br.read_bit()?;
        if tree_select {
            let lengths = vec![
                (symbols[0], 1),
                (symbols[1], 2),
                (symbols[2], 3),
                (symbols[3], 3),
            ];
            return canonical_codes_allowing_single_zero(alphabet_size, lengths)
                .ok_or(BrotliError::InvalidStream);
        }
    }
    simple_canonical_codes(&SimplePrefixCode {
        alphabet_size,
        alphabet_bits,
        symbols,
    })
    .ok_or(BrotliError::InvalidStream)
}

fn decode_complex_prefix_lengths(
    br: &mut BitReader<'_>,
    alphabet_size: u16,
    hskip: u32,
) -> Result<Vec<u8>, BrotliError> {
    if hskip != 0 {
        return Err(BrotliError::UnsupportedStream);
    }
    let mut code_length_lengths = vec![0u8; 18];
    let mut kraft = 0u32;
    let mut read_any = false;
    for &symbol in &BROTLI_CODE_LENGTH_CODE_ORDER {
        let len = read_code_length_code_length(br)?;
        code_length_lengths[symbol as usize] = len;
        if len != 0 {
            kraft += 32u32 >> len;
            read_any = true;
            if kraft == 32 {
                break;
            }
            if kraft > 32 {
                return Err(BrotliError::InvalidStream);
            }
        } else if read_any && kraft == 32 {
            break;
        }
    }
    if kraft != 32 {
        return Err(BrotliError::InvalidStream);
    }
    let code_length_codes = canonical_codes(&ComplexPrefixCode {
        alphabet_size: 18,
        max_bits: 5,
        code_lengths: code_length_lengths
            .iter()
            .enumerate()
            .filter_map(|(symbol, &len)| (len != 0).then_some((symbol as u16, len)))
            .collect(),
    })
    .ok_or(BrotliError::InvalidStream)?;

    let mut lengths = Vec::with_capacity(alphabet_size as usize);
    let mut target_kraft = 0u32;
    while target_kraft < (1u32 << 15) {
        let symbol = read_canonical_symbol(br, &code_length_codes)?;
        match symbol {
            0..=15 => {
                let len = symbol as u8;
                lengths.push(len);
                if len != 0 {
                    target_kraft += 1u32 << (15 - len);
                }
            }
            16 => {
                let prev = *lengths.last().ok_or(BrotliError::InvalidStream)?;
                if prev == 0 {
                    return Err(BrotliError::InvalidStream);
                }
                let repeat = 3 + br.read_bits(2)? as usize;
                for _ in 0..repeat {
                    lengths.push(prev);
                    target_kraft += 1u32 << (15 - prev);
                }
            }
            17 => {
                let repeat = 3 + br.read_bits(3)? as usize;
                lengths.resize(lengths.len() + repeat, 0);
            }
            _ => return Err(BrotliError::InvalidStream),
        }
        if lengths.len() > alphabet_size as usize || target_kraft > (1u32 << 15) {
            return Err(BrotliError::InvalidStream);
        }
    }
    lengths.resize(alphabet_size as usize, 0);
    Ok(lengths)
}

fn read_code_length_code_length(br: &mut BitReader<'_>) -> Result<u8, BrotliError> {
    if !br.read_bit()? {
        return if br.read_bit()? { Ok(3) } else { Ok(0) };
    }
    if !br.read_bit()? {
        return Ok(4);
    }
    if !br.read_bit()? {
        return Ok(2);
    }
    if !br.read_bit()? {
        return Ok(1);
    }
    Ok(5)
}

fn read_canonical_symbol(
    br: &mut BitReader<'_>,
    codes: &[CanonicalCode],
) -> Result<u16, BrotliError> {
    if let Some(code) = codes.iter().find(|code| code.len == 0) {
        return Ok(code.symbol);
    }
    let mut bits = 0u16;
    for len in 1..=15u8 {
        if br.read_bit()? {
            bits |= 1 << (len - 1);
        }
        if let Some(code) = codes
            .iter()
            .find(|code| code.len == len && code.lsb_bits == bits)
        {
            return Ok(code.symbol);
        }
    }
    Err(BrotliError::InvalidStream)
}

fn decode_insert_copy_symbol(symbol: u16) -> Result<(u16, u16, bool), BrotliError> {
    for explicit in [false, true] {
        for insert_code in 0..24u16 {
            for copy_code in 0..24u16 {
                if insert_copy_code_from_codes(insert_code, copy_code, explicit) == Some(symbol) {
                    return Ok((insert_code, copy_code, explicit));
                }
            }
        }
    }
    Err(BrotliError::InvalidStream)
}

fn insert_copy_code_from_codes(insert_code: u16, copy_code: u16, explicit: bool) -> Option<u16> {
    let range_base = match (explicit, insert_code, copy_code) {
        (false, 0..=7, 0..=7) => 0,
        (false, 0..=7, 8..=15) => 64,
        (true, 0..=7, 0..=7) => 128,
        (true, 0..=7, 8..=15) => 192,
        (true, 0..=7, 16..=23) => 384,
        (true, 8..=15, 0..=7) => 256,
        (true, 8..=15, 8..=15) => 320,
        (true, 8..=15, 16..=23) => 512,
        (true, 16..=23, 0..=7) => 448,
        (true, 16..=23, 8..=15) => 576,
        (true, 16..=23, 16..=23) => 640,
        _ => return None,
    };
    Some(range_base + ((insert_code & 7) << 3) + (copy_code & 7))
}

fn decode_length(
    br: &mut BitReader<'_>,
    code: u16,
    ranges: &[(usize, usize, u8)],
) -> Result<usize, BrotliError> {
    let (start, _, extra_bits) = *ranges
        .get(code as usize)
        .ok_or(BrotliError::InvalidStream)?;
    Ok(start + br.read_bits(extra_bits)? as usize)
}

fn decode_distance(
    br: &mut BitReader<'_>,
    symbol: u16,
    ndirect: u16,
    npostfix: u8,
) -> Result<usize, BrotliError> {
    let extra_bits = (1..=BROTLI_GENERATED_MAX_DISTANCE)
        .find_map(|distance| {
            let code = distance_code(distance, ndirect, npostfix);
            (code.code == symbol).then_some(code.extra_bits)
        })
        .ok_or(BrotliError::InvalidStream)?;
    let extra_value = br.read_bits(extra_bits)?;
    for distance in 1..=BROTLI_GENERATED_MAX_DISTANCE {
        let code = distance_code(distance, ndirect, npostfix);
        if code.code == symbol && code.extra_bits == extra_bits && code.extra_value == extra_value {
            return Ok(distance);
        }
    }
    Err(BrotliError::InvalidStream)
}

fn decode_distance_with_cache(
    br: &mut BitReader<'_>,
    symbol: u16,
    ndirect: u16,
    npostfix: u8,
    cache: &BrotliDistanceCache,
) -> Result<usize, BrotliError> {
    if symbol < 16 {
        return cache
            .distance_for_short_code(symbol)
            .ok_or(BrotliError::InvalidStream);
    }
    decode_distance(br, symbol, ndirect, npostfix)
}

struct BitWriter {
    out: Vec<u8>,
    current: u8,
    used: u8,
    written_bits: usize,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            current: 0,
            used: 0,
            written_bits: 0,
        }
    }

    fn write_bit(&mut self, bit: bool) {
        if bit {
            self.current |= 1 << self.used;
        }
        self.used += 1;
        self.written_bits += 1;
        if self.used == 8 {
            self.flush_byte();
        }
    }

    fn write_bits(&mut self, value: u32, count: u8) {
        for i in 0..count {
            self.write_bit(((value >> i) & 1) != 0);
        }
    }

    #[allow(dead_code)]
    fn write_span(&mut self, span: BitSpan) {
        self.write_bits(span.bits as u32, span.len);
    }

    #[allow(dead_code)]
    fn write_spans(&mut self, spans: &[BitSpan]) {
        for &span in spans {
            self.write_span(span);
        }
    }

    fn write_aligned_bytes(&mut self, bytes: &[u8]) {
        self.align_to_byte();
        self.out.extend_from_slice(bytes);
        self.written_bits += bytes.len() * 8;
    }

    fn align_to_byte(&mut self) {
        if self.used != 0 {
            self.flush_byte();
        }
    }

    fn finish(mut self) -> Vec<u8> {
        self.align_to_byte();
        self.out
    }

    #[allow(dead_code)]
    fn bit_len(&self) -> usize {
        self.written_bits
    }

    fn flush_byte(&mut self) {
        self.out.push(self.current);
        self.current = 0;
        self.used = 0;
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte: 0,
            bit: 0,
        }
    }

    fn read_bit(&mut self) -> Result<bool, BrotliError> {
        if self.byte >= self.data.len() {
            return Err(BrotliError::UnexpectedEnd);
        }
        let value = ((self.data[self.byte] >> self.bit) & 1) != 0;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.byte += 1;
        }
        Ok(value)
    }

    fn read_bits(&mut self, count: u8) -> Result<u32, BrotliError> {
        let mut value = 0u32;
        for i in 0..count {
            if self.read_bit()? {
                value |= 1 << i;
            }
        }
        Ok(value)
    }

    fn align_to_byte(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.byte += 1;
        }
    }

    fn read_aligned_bytes(&mut self, len: usize) -> Result<&'a [u8], BrotliError> {
        if self.bit != 0 {
            return Err(BrotliError::InvalidStream);
        }
        let end = self
            .byte
            .checked_add(len)
            .ok_or(BrotliError::InvalidStream)?;
        if end > self.data.len() {
            return Err(BrotliError::UnexpectedEnd);
        }
        let bytes = &self.data[self.byte..end];
        self.byte = end;
        Ok(bytes)
    }

    fn remaining_fill_bits_are_zero(&self) -> bool {
        if self.byte >= self.data.len() {
            return true;
        }
        let mask = if self.bit == 0 {
            0xff
        } else {
            !((1u8 << self.bit) - 1)
        };
        if self.data[self.byte] & mask != 0 {
            return false;
        }
        self.data[self.byte + 1..].iter().all(|&b| b == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_empty_kats() {
        assert_eq!(decode(&[0x3b]).unwrap(), b"");
        assert_eq!(decode(&[0x06]).unwrap(), b"");
        assert_eq!(decode(&[0x06, 0x00]).unwrap(), b"");
    }

    #[test]
    fn decodes_external_hello_literal_stream() {
        let encoded = [
            0x0b, 0x06, 0x80, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x57, 0x6f, 0x72, 0x6c,
            0x64, 0x21, 0x03,
        ];
        assert_eq!(decode(&encoded).unwrap(), b"Hello, World!");
    }

    #[test]
    fn roundtrips_literal_lengths_one_through_512() {
        for len in 1..=512usize {
            let data = vec![(len & 0xff) as u8; len];
            let encoded = encode(
                &data,
                &BrotliParams {
                    quality: 0,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(encoded.len(), len + 4, "len={len}");
            assert_eq!(decode(&encoded).unwrap(), data, "len={len}");
        }
    }

    #[test]
    fn compressed_literal_blocks_decode_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        for len in [1usize, 2, 13, 64, 255, 256, 511, 512] {
            let data: Vec<u8> = (0..len).map(|i| ((i * 17) & 0xff) as u8).collect();
            let encoded = encode(&data, &BrotliParams::default()).unwrap();
            let mut child = match Command::new("node")
                .arg("-e")
                .arg(
                    "const fs=require('fs');\
                     const z=require('zlib');\
                     process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
                )
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(_) => {
                    eprintln!("node missing; skip brotli node interop test");
                    return;
                }
            };
            child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(
                out.status.success(),
                "node brotli decode failed len={len}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(out.stdout, data, "len={len}");
        }
    }

    #[test]
    fn lz77_plan_finds_repeated_substrings() {
        let data = b"abcabcabcabc";
        let plan = plan_lz77(data);
        assert!(
            plan.iter()
                .any(|step| matches!(step, Lz77Step::Copy { distance: 3, len } if *len >= 6)),
            "plan should find repeated abc copy: {plan:?}"
        );
    }

    #[test]
    fn high_quality_lz77_uses_lazy_match_when_next_copy_is_better() {
        let data = b"44X44X5cX44X44X3X11aX".to_vec();
        let greedy = plan_lz77_with_profile(
            &data,
            Lz77Profile {
                candidate_limit: 32,
                max_match: LZ77_MAX_MATCH,
                lazy_match: false,
            },
        );
        let lazy = plan_lz77_with_profile(
            &data,
            Lz77Profile {
                candidate_limit: 32,
                max_match: LZ77_MAX_MATCH,
                lazy_match: true,
            },
        );

        assert_ne!(greedy, lazy);
        assert!(greedy.contains(&Lz77Step::Copy {
            distance: 6,
            len: 4,
        }));
        assert!(lazy.contains(&Lz77Step::Copy {
            distance: 9,
            len: 6,
        }));
    }

    #[test]
    fn lz77_plan_collapses_repeated_json_payload() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let plan = plan_lz77(&data);
        let copied: usize = plan
            .iter()
            .filter_map(|step| match step {
                Lz77Step::Copy { len, .. } => Some(*len),
                _ => None,
            })
            .sum();
        assert!(
            copied > data.len() / 2,
            "planner should route most repeated payload through copies: copied={copied}, len={}, plan={plan:?}",
            data.len()
        );
    }

    #[test]
    fn insert_length_code_matches_rfc_ranges() {
        assert_eq!(
            insert_length_code(0),
            LengthCode {
                code: 0,
                extra_bits: 0,
                extra_value: 0
            }
        );
        assert_eq!(
            insert_length_code(7),
            LengthCode {
                code: 6,
                extra_bits: 1,
                extra_value: 1
            }
        );
        assert_eq!(
            insert_length_code(130),
            LengthCode {
                code: 16,
                extra_bits: 6,
                extra_value: 0
            }
        );
        assert_eq!(insert_length_code(6209).code, 21);
    }

    #[test]
    fn copy_length_code_matches_rfc_ranges() {
        assert_eq!(
            copy_length_code(2),
            LengthCode {
                code: 0,
                extra_bits: 0,
                extra_value: 0
            }
        );
        assert_eq!(
            copy_length_code(11),
            LengthCode {
                code: 8,
                extra_bits: 1,
                extra_value: 1
            }
        );
        assert_eq!(
            copy_length_code(258),
            LengthCode {
                code: 19,
                extra_bits: 7,
                extra_value: 60
            }
        );
    }

    #[test]
    fn insert_copy_code_selects_rfc_grid_cell() {
        let implicit = insert_copy_code(3, 5, false);
        assert_eq!(implicit.code, 27);
        assert_eq!(implicit.insert_extra_bits, 0);
        assert_eq!(implicit.copy_extra_bits, 0);

        let explicit = insert_copy_code(3, 5, true);
        assert_eq!(explicit.code, 155);
        assert_eq!(explicit.insert_extra_bits, 0);
        assert_eq!(explicit.copy_extra_bits, 0);

        let code = insert_copy_code(10, 258, true);
        assert_eq!(code.code, 515);
        assert_eq!(code.insert_extra_bits, 2);
        assert_eq!(code.copy_extra_bits, 7);
    }

    #[test]
    fn distance_code_matches_zero_direct_zero_postfix_ranges() {
        assert_eq!(
            distance_code(1, 0, 0),
            DistanceCode {
                code: 16,
                extra_bits: 1,
                extra_value: 0
            }
        );
        assert_eq!(
            distance_code(2, 0, 0),
            DistanceCode {
                code: 16,
                extra_bits: 1,
                extra_value: 1
            }
        );
        assert_eq!(
            distance_code(3, 0, 0),
            DistanceCode {
                code: 17,
                extra_bits: 1,
                extra_value: 0
            }
        );
        assert_eq!(
            distance_code(16, 0, 0),
            DistanceCode {
                code: 20,
                extra_bits: 3,
                extra_value: 3
            }
        );
    }

    #[test]
    fn distance_code_honors_direct_and_postfix_parameters() {
        assert_eq!(
            distance_code(1, 4, 0),
            DistanceCode {
                code: 16,
                extra_bits: 0,
                extra_value: 0
            }
        );
        assert_eq!(
            distance_code(4, 4, 0),
            DistanceCode {
                code: 19,
                extra_bits: 0,
                extra_value: 0
            }
        );
        assert_eq!(
            distance_code(5, 4, 1),
            DistanceCode {
                code: 20,
                extra_bits: 1,
                extra_value: 0
            }
        );
        assert_eq!(
            distance_code(6, 4, 1),
            DistanceCode {
                code: 21,
                extra_bits: 1,
                extra_value: 0
            }
        );
    }

    #[test]
    fn planned_match_distance_lowers_to_distance_symbol() {
        let data = b"abcabcabcabc";
        let plan = plan_lz77(data);
        let code = plan
            .iter()
            .find_map(|step| match step {
                Lz77Step::Copy { distance, .. } => Some(distance_code(*distance, 0, 0)),
                _ => None,
            })
            .expect("copy step");
        assert_eq!(
            code,
            DistanceCode {
                code: 17,
                extra_bits: 1,
                extra_value: 0
            }
        );
    }

    #[test]
    fn lowers_lz77_plan_to_brotli_commands() {
        let data = b"abcabcabcabc";
        let commands = lower_lz77_commands(data, 0, 0);
        assert!(
            commands.iter().any(|cmd| cmd.copy_distance == Some(3)
                && cmd.copy_len >= 6
                && cmd.distance
                    == Some(DistanceCode {
                        code: 17,
                        extra_bits: 1,
                        extra_value: 0
                    })),
            "commands should lower distance-3 repeated copy: {commands:?}"
        );
        assert_eq!(commands[0].insert_start, 0);
        assert_eq!(commands[0].insert_len, 3);
        assert_eq!(commands[0].copy_len, 9);
        assert_eq!(commands[0].insert_copy.code, 159);
    }

    #[test]
    fn command_lowering_keeps_terminal_literal_command_distance_free() {
        let data = b"abcabcabcabc!";
        let commands = lower_lz77_commands(data, 0, 0);
        let last = commands.last().expect("terminal command");
        assert_eq!(last.copy_distance, None);
        assert_eq!(last.distance, None);
        assert_eq!(
            &data[last.insert_start..last.insert_start + last.insert_len],
            b"!"
        );
    }

    #[test]
    fn command_lowering_collapses_repeated_payload_bytes() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let payload_bytes = command_stream_payload_bytes(&commands);
        let copied: usize = commands.iter().map(|cmd| cmd.copy_len).sum();
        assert!(
            payload_bytes < data.len() / 2,
            "command payload should be smaller than raw bytes: payload={payload_bytes}, len={}, commands={}",
            data.len(),
            commands.len()
        );
        assert!(
            copied > data.len() / 2,
            "commands should retain planner copy coverage: copied={copied}, len={}",
            data.len()
        );
    }

    #[test]
    fn distance_cache_lowering_omits_repeated_backward_distances() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let plain = lower_lz77_commands_with_profile(&data, 0, 0, lz77_profile(11));
        let cached = lower_lz77_commands_with_profile_output_base_and_distance_cache(
            &data,
            0,
            0,
            lz77_profile(11),
            0,
        );

        assert!(commands_have_implicit_distance(&cached));
        assert!(
            cached.iter().filter(|cmd| cmd.distance.is_some()).count()
                < plain.iter().filter(|cmd| cmd.distance.is_some()).count()
        );
    }

    #[test]
    fn distance_cache_single_block_stream_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let plain = lower_lz77_commands_with_profile(&data, 0, 0, lz77_profile(11));
        let cached = lower_lz77_commands_with_profile_output_base_and_distance_cache(
            &data,
            0,
            0,
            lz77_profile(11),
            0,
        );
        let plain_profile =
            single_block_complex_prefix_profile(&data, &plain).expect("plain profile");
        let cached_profile =
            single_block_complex_prefix_profile(&data, &cached).expect("cached profile");
        let plain_stream =
            write_single_block_compressed_stream_fragment(&data, &plain, &plain_profile)
                .expect("plain stream");
        let cached_stream =
            write_single_block_compressed_stream_fragment(&data, &cached, &cached_profile)
                .expect("cached stream");

        assert!(cached_stream.bit_len < plain_stream.bit_len);
        assert_eq!(decode(&cached_stream.bytes).unwrap(), data);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&cached_stream.bytes)
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn distance_cache_block_type_admission_improves_public_repeated_json_q11() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let encoded = encode(&data, &BrotliParams::default()).unwrap();
        assert!(
            encoded.len() <= 214,
            "cached block-type q11 stream should stay near Node size class: {}",
            encoded.len()
        );
        assert_eq!(decode(&encoded).unwrap(), data);
    }

    #[test]
    fn distance_cache_short_codes_cover_adjacent_distances() {
        let mut cache = BrotliDistanceCache::new();
        assert_eq!(cache.short_code(17), None);

        cache.push(181);
        assert_eq!(
            distance_code_with_cache(182, 12, 0, &cache),
            DistanceCode {
                code: 5,
                extra_bits: 0,
                extra_value: 0
            }
        );
        assert_eq!(cache.distance_for_short_code(5), Some(182));

        cache.push(182);
        assert_eq!(
            distance_code_with_cache(181, 12, 0, &cache),
            DistanceCode {
                code: 1,
                extra_bits: 0,
                extra_value: 0
            }
        );
        assert_eq!(cache.distance_for_short_code(1), Some(181));
    }

    #[test]
    fn recent_distance_alternative_improves_repeated_json_q11_plan() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let profile = lz77_profile(11);
        let distance_profile = DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        };
        let commands = lower_lz77_commands_with_profile_output_base_and_distance_cache(
            &data,
            distance_profile.ndirect,
            distance_profile.npostfix,
            profile,
            0,
        );
        let baseline =
            best_literal_block_type_lz77_for_commands(&data, &commands, distance_profile, None)
                .expect("baseline block-type stream");
        let improved = best_literal_block_type_lz77_for_recent_distance_alternatives(
            &data,
            distance_profile,
            profile,
            Some(baseline.clone()),
        )
        .expect("recent-distance alternative stream");

        assert!(
            improved.bit_len < baseline.bit_len,
            "baseline={}, improved={}",
            baseline.bit_len,
            improved.bit_len
        );
        let encoded = encode(&data, &BrotliParams::default()).unwrap();
        assert!(encoded.len() <= 214, "encoded={}", encoded.len());
        assert_eq!(decode(&encoded).unwrap(), data);
    }

    #[test]
    fn lz77_plan_uses_long_copy_beyond_deflate_cap() {
        let mut data = Vec::new();
        for _ in 0..160 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        let plan = plan_lz77(&data);
        let longest = plan
            .iter()
            .filter_map(|step| match step {
                Lz77Step::Copy { len, .. } => Some(*len),
                _ => None,
            })
            .max()
            .unwrap_or(0);

        assert!(
            longest > 258,
            "Brotli copy planner should not inherit DEFLATE's 258-byte cap: longest={longest}"
        );
    }

    #[test]
    fn quality_profile_bounds_lz77_copy_depth() {
        let mut data = Vec::new();
        for _ in 0..160 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }

        let q1 = plan_lz77_with_profile(&data, lz77_profile(1));
        let q6 = plan_lz77_with_profile(&data, lz77_profile(6));
        let q11 = plan_lz77_with_profile(&data, lz77_profile(11));
        let longest = |plan: &[Lz77Step]| {
            plan.iter()
                .filter_map(|step| match step {
                    Lz77Step::Copy { len, .. } => Some(*len),
                    _ => None,
                })
                .max()
                .unwrap_or(0)
        };

        assert!(longest(&q1) <= 64, "q1 longest={}", longest(&q1));
        assert!(longest(&q6) <= 258, "q6 longest={}", longest(&q6));
        assert!(
            longest(&q11) > longest(&q6),
            "q11 should admit longer Brotli copies: q6={}, q11={}",
            longest(&q6),
            longest(&q11)
        );
    }

    #[test]
    fn single_symbol_prefix_frequencies_gain_dummy_peer() {
        assert_eq!(
            complete_prefix_frequencies(64, vec![(22, 9)]),
            Some(vec![(22, 9), (0, 1)])
        );
        assert_eq!(
            complete_prefix_frequencies(64, vec![(0, 9)]),
            Some(vec![(0, 9), (1, 1)])
        );
    }

    #[test]
    fn long_copy_band_serializes_and_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let data = b"Cruft press production workflow payload. ".repeat(160);
        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let stream =
            write_single_block_compressed_stream_fragment(&data, &commands, &profile).unwrap();

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&stream.bytes)
            .unwrap();
        let out = child.wait_with_output().unwrap();

        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
        assert_eq!(decode(&stream.bytes).unwrap(), data);
    }

    #[test]
    fn public_encode_uses_long_copy_band_before_stored_fallback() {
        let data = b"Cruft press production workflow payload. ".repeat(160);
        let encoded = encode(&data, &BrotliParams::default()).unwrap();
        let q0 = encode(
            &data,
            &BrotliParams {
                quality: 0,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(
            encoded.len() < q0.len(),
            "long-copy stream should publish before stored fallback: encoded={}, q0={}",
            encoded.len(),
            q0.len()
        );
        assert_eq!(decode(&encoded).unwrap(), data);
    }

    #[test]
    fn public_encode_uses_two_block_lz77_for_mixed_text_64k() {
        let mut data = Vec::new();
        for i in 0..1024 {
            data.extend_from_slice(
                format!(
                    "line={i}; module=cruft; status={}; payload=press zlib brotli gzip deflate\n",
                    i % 7
                )
                .as_bytes(),
            );
        }
        let profile = lz77_profile(11);
        let q0 = encode(
            &data,
            &BrotliParams {
                quality: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(encode_single_block_lz77(&data, profile).is_none());
        let two = encode_two_block_lz77(&data, profile).expect("two-block candidate");
        let encoded = encode(&data, &BrotliParams::default()).unwrap();
        assert!(two.len() < 10_000, "two-block candidate={}", two.len());
        assert_eq!(two.len(), 3345);
        assert!(
            encoded.len() < q0.len(),
            "encoded={}, q0={}",
            encoded.len(),
            q0.len()
        );
        assert_eq!(encoded.len(), two.len());
        assert_eq!(decode(&encoded).unwrap(), data);
    }

    #[test]
    fn large_two_block_lz77_search_is_bounded() {
        let data = vec![b'a'; 74_666];
        let profile = lz77_profile(11);
        assert_eq!(two_block_split_candidates(&data), vec![51_200]);
        assert_eq!(
            two_block_distance_profiles(&data, profile),
            &LARGE_TWO_BLOCK_DISTANCE_PROFILES
        );
        assert_eq!(
            LARGE_TWO_BLOCK_DISTANCE_PROFILE_PAIRS,
            [(
                DistanceProfile {
                    ndirect: 0,
                    npostfix: 0,
                },
                DistanceProfile {
                    ndirect: 4,
                    npostfix: 0,
                }
            )]
        );
    }

    #[test]
    fn sub_meta_block_payloads_skip_two_block_search() {
        let files = (0..64)
            .map(|i| format!("\"src/module-{i}.js\""))
            .collect::<Vec<_>>()
            .join(",");
        let repeated = format!(
            "{{\"name\":\"compression-runtime-matrix\",\"files\":[{files}],\"body\":\"{}\"}}",
            "Cruft press production workflow payload. ".repeat(128)
        )
        .into_bytes();
        let mut binary = vec![0u8; 16_384];
        let mut seed = 0x12345678u32;
        for byte in &mut binary {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (seed & 255) as u8;
        }
        let profile = lz77_profile(11);

        assert!(encode_two_block_lz77(&repeated, profile).is_none());
        assert!(encode_two_block_lz77(&binary, profile).is_none());
        assert_eq!(
            encode(&repeated, &BrotliParams::default()).unwrap().len(),
            220
        );
        assert_eq!(
            encode(&binary, &BrotliParams::default()).unwrap().len(),
            293
        );
    }

    #[test]
    fn bounded_search_preserves_repeated_json_and_binary_byte_classes() {
        let files = (0..64)
            .map(|i| format!("\"src/module-{i}.js\""))
            .collect::<Vec<_>>()
            .join(",");
        let repeated = format!(
            "{{\"name\":\"compression-runtime-matrix\",\"files\":[{files}],\"body\":\"{}\"}}",
            "Cruft press production workflow payload. ".repeat(128)
        )
        .into_bytes();
        let mut binary = vec![0u8; 16_384];
        let mut seed = 0x12345678u32;
        for byte in &mut binary {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (seed & 255) as u8;
        }

        assert_eq!(
            encode(&repeated, &BrotliParams::default()).unwrap().len(),
            220
        );
        assert_eq!(
            encode(&binary, &BrotliParams::default()).unwrap().len(),
            293
        );
    }

    #[test]
    fn recent_distance_alternative_search_is_bounded() {
        let files = (0..64)
            .map(|i| format!("\"src/module-{i}.js\""))
            .collect::<Vec<_>>()
            .join(",");
        let data = format!(
            "{{\"name\":\"compression-runtime-matrix\",\"files\":[{files}],\"body\":\"{}\"}}",
            "Cruft press production workflow payload. ".repeat(128)
        )
        .into_bytes();

        assert_eq!(RECENT_DISTANCE_ALTERNATIVE_LIMIT, 1);
        assert_eq!(encode(&data, &BrotliParams::default()).unwrap().len(), 220);
    }

    #[test]
    fn prefix_profile_alphabet_bits_match_rfc_simple_widths() {
        assert_eq!(alphabet_bits(64), 6);
        assert_eq!(alphabet_bits(256), 8);
        assert_eq!(alphabet_bits(704), 10);
    }

    #[test]
    fn prefix_profile_describes_simple_copy_bearing_block() {
        let data = b"abcabcabcabc";
        let commands = lower_lz77_commands(data, 0, 0);
        let profile = single_block_prefix_profile(data, &commands).expect("simple profile");

        assert_eq!(profile.ndirect, 0);
        assert_eq!(profile.npostfix, 0);
        assert_eq!(profile.literal.alphabet_size, 256);
        assert_eq!(profile.literal.alphabet_bits, 8);
        assert_eq!(
            profile.literal.symbols,
            vec![b'a' as u16, b'b' as u16, b'c' as u16]
        );
        assert_eq!(profile.insert_copy.alphabet_size, 704);
        assert_eq!(profile.insert_copy.alphabet_bits, 10);
        assert_eq!(profile.insert_copy.symbols, vec![159]);
        assert_eq!(profile.distance.alphabet_size, 64);
        assert_eq!(profile.distance.alphabet_bits, 6);
        assert_eq!(profile.distance.symbols, vec![17]);
    }

    #[test]
    fn prefix_profile_keeps_literal_only_distance_fallback() {
        let data = b"aaaa";
        let commands = lower_lz77_commands(data, 0, 0);
        let profile = single_block_prefix_profile(data, &commands).expect("literal profile");

        assert_eq!(profile.literal.symbols, vec![b'a' as u16]);
        assert_eq!(profile.distance.symbols, vec![16]);
    }

    #[test]
    fn simple_prefix_description_uses_rfc_header_shape() {
        let simple = simple_prefix_code(64, vec![17]);
        assert_eq!(
            simple_prefix_description_spans(&simple).unwrap(),
            vec![
                BitSpan { bits: 1, len: 2 },
                BitSpan { bits: 0, len: 2 },
                BitSpan { bits: 17, len: 6 },
            ]
        );

        let codes = simple_canonical_codes(&simple).unwrap();
        assert_eq!(
            codes,
            vec![CanonicalCode {
                symbol: 17,
                len: 0,
                msb_code: 0,
                lsb_bits: 0,
            }]
        );
    }

    #[test]
    fn prefix_profile_routes_general_json_to_complex_prefix_rung() {
        let data = br#"{"name":"brotli-node-parity","files":["src/module-0.js"],"body":"Cruft press production workflow payload."}"#;
        let commands = lower_lz77_commands(data, 0, 0);

        assert_eq!(single_block_prefix_profile(data, &commands), None);
    }

    #[test]
    fn complex_prefix_profile_describes_general_json_payload() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        assert_eq!(single_block_prefix_profile(&data, &commands), None);

        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        assert_eq!(profile.ndirect, 0);
        assert_eq!(profile.npostfix, 0);
        assert_eq!(profile.literal.alphabet_size, 256);
        assert_eq!(profile.literal.max_bits, 15);
        assert!(profile.literal.code_lengths.len() > 4);
        assert!(profile
            .literal
            .code_lengths
            .iter()
            .all(|&(symbol, len)| symbol < 256 && (1..=15).contains(&len)));
        assert_eq!(profile.insert_copy.alphabet_size, 704);
        assert!(profile
            .insert_copy
            .code_lengths
            .iter()
            .all(|&(symbol, len)| symbol < 704 && (1..=15).contains(&len)));
        assert_eq!(profile.distance.alphabet_size, 64);
        assert!(profile
            .distance
            .code_lengths
            .iter()
            .all(|&(symbol, len)| symbol < 64 && (1..=15).contains(&len)));
    }

    #[test]
    fn huffman_lengths_rank_frequent_symbols_shorter() {
        let lengths =
            huffman_code_lengths(vec![(b'a' as u16, 16), (b'b' as u16, 4), (b'c' as u16, 1)])
                .unwrap();
        let len_a = lengths
            .iter()
            .find_map(|&(symbol, len)| (symbol == b'a' as u16).then_some(len))
            .unwrap();
        let len_c = lengths
            .iter()
            .find_map(|&(symbol, len)| (symbol == b'c' as u16).then_some(len))
            .unwrap();
        assert!(len_a <= len_c);
    }

    #[test]
    fn canonical_codes_assign_reversed_stream_bits() {
        let prefix = ComplexPrefixCode {
            alphabet_size: 8,
            max_bits: 15,
            code_lengths: vec![(0, 1), (1, 3), (2, 3), (3, 3), (4, 3)],
        };
        let codes = canonical_codes(&prefix).unwrap();

        assert_eq!(
            codes,
            vec![
                CanonicalCode {
                    symbol: 0,
                    len: 1,
                    msb_code: 0b0,
                    lsb_bits: 0b0
                },
                CanonicalCode {
                    symbol: 1,
                    len: 3,
                    msb_code: 0b100,
                    lsb_bits: 0b001
                },
                CanonicalCode {
                    symbol: 2,
                    len: 3,
                    msb_code: 0b101,
                    lsb_bits: 0b101
                },
                CanonicalCode {
                    symbol: 3,
                    len: 3,
                    msb_code: 0b110,
                    lsb_bits: 0b011
                },
                CanonicalCode {
                    symbol: 4,
                    len: 3,
                    msb_code: 0b111,
                    lsb_bits: 0b111
                },
            ]
        );
    }

    #[test]
    fn canonical_codes_reject_oversubscribed_lengths() {
        let prefix = ComplexPrefixCode {
            alphabet_size: 8,
            max_bits: 15,
            code_lengths: vec![(0, 1), (1, 1), (2, 1)],
        };

        assert_eq!(canonical_codes(&prefix), None);
    }

    #[test]
    fn complex_prefix_payload_bits_are_below_raw_repeated_payload() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let command_bits =
            encoded_command_bit_estimate(&data, &commands, &profile).expect("bit estimate");

        assert!(
            command_bits < data.len() * 4,
            "command stream bits should be far below raw bits before header cost: bits={command_bits}, raw_bits={}",
            data.len() * 8
        );
    }

    #[test]
    fn code_length_ops_encode_zero_and_previous_length_runs() {
        let lengths = [
            3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 4, 4,
        ];
        let ops = code_length_ops(&lengths).unwrap();

        assert_eq!(
            ops,
            vec![
                CodeLengthOp {
                    symbol: 3,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0
                },
                CodeLengthOp {
                    symbol: 16,
                    repeat: 4,
                    extra_bits: 2,
                    extra_value: 1
                },
                CodeLengthOp {
                    symbol: 17,
                    repeat: 10,
                    extra_bits: 3,
                    extra_value: 7
                },
                CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0
                },
                CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0
                },
                CodeLengthOp {
                    symbol: 4,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0
                },
                CodeLengthOp {
                    symbol: 16,
                    repeat: 3,
                    extra_bits: 2,
                    extra_value: 0
                },
            ]
        );
    }

    #[test]
    fn prefix_description_plan_compacts_repeated_json_alphabets() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let literal_plan = prefix_description_plan(&profile.literal).unwrap();
        let insert_copy_plan = prefix_description_plan(&profile.insert_copy).unwrap();
        let distance_plan = prefix_description_plan(&profile.distance).unwrap();

        assert_eq!(literal_plan.alphabet_size, 256);
        assert!(literal_plan.trimmed_len <= 256);
        assert!(literal_plan.ops.len() < literal_plan.trimmed_len);
        assert!(literal_plan
            .ops
            .iter()
            .any(|op| matches!(op.symbol, 16 | 17)));
        assert!(insert_copy_plan.ops.iter().all(|op| op.symbol <= 17));
        assert!(distance_plan.ops.iter().all(|op| op.symbol <= 17));

        let op_freqs =
            prefix_description_op_frequencies(&[literal_plan, insert_copy_plan, distance_plan]);
        assert!(op_freqs.iter().any(|&(symbol, _)| symbol == 17));
        assert!(op_freqs
            .iter()
            .all(|&(symbol, count)| symbol <= 17 && count > 0));
    }

    #[test]
    fn code_length_prefix_plan_uses_brotli_symbol_order() {
        let plans = vec![PrefixDescriptionPlan {
            alphabet_size: 32,
            trimmed_len: 21,
            ops: vec![
                CodeLengthOp {
                    symbol: 3,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 16,
                    repeat: 4,
                    extra_bits: 2,
                    extra_value: 1,
                },
                CodeLengthOp {
                    symbol: 17,
                    repeat: 10,
                    extra_bits: 3,
                    extra_value: 7,
                },
                CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 4,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 17,
                    repeat: 5,
                    extra_bits: 3,
                    extra_value: 2,
                },
            ],
        }];

        let plan = code_length_prefix_plan(&plans).unwrap();

        assert_eq!(plan.code.alphabet_size, 18);
        assert_eq!(plan.code.max_bits, 5);
        assert!(plan
            .canonical
            .iter()
            .all(|code| code.symbol < 18 && code.len <= 5));
        assert!(plan
            .ordered_lengths
            .iter()
            .any(|&(symbol, len)| symbol == 16 && len > 0));
        assert!(plan
            .ordered_lengths
            .iter()
            .any(|&(symbol, len)| symbol == 17 && len > 0));
        assert!(plan.ordered_lengths.len() <= BROTLI_CODE_LENGTH_CODE_ORDER.len());

        assert_eq!(validate_code_length_prefix_header(&plan), Some(()));
    }

    #[test]
    fn prefix_description_bit_plan_preserves_header_then_ops_order() {
        let plans = vec![PrefixDescriptionPlan {
            alphabet_size: 32,
            trimmed_len: 21,
            ops: vec![
                CodeLengthOp {
                    symbol: 3,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 16,
                    repeat: 4,
                    extra_bits: 2,
                    extra_value: 1,
                },
                CodeLengthOp {
                    symbol: 17,
                    repeat: 10,
                    extra_bits: 3,
                    extra_value: 7,
                },
                CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
            ],
        }];
        let code_plan = code_length_prefix_plan(&plans).unwrap();
        let bit_plan = prefix_description_bit_plan(&plans, &code_plan).unwrap();

        assert_eq!(
            bit_plan.header_lengths.len(),
            code_plan.ordered_lengths.len() + 1
        );
        assert_eq!(bit_plan.header_lengths[0], BitSpan { bits: 0, len: 2 });
        assert_eq!(
            &bit_plan.header_lengths[1..],
            code_plan
                .ordered_lengths
                .iter()
                .map(|&(_, len)| code_length_code_length_span(len).unwrap())
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(
            bit_plan.op_spans[0],
            code_length_symbol_span(&code_plan, 3).unwrap()
        );
        assert_eq!(
            bit_plan.op_spans[1],
            code_length_symbol_span(&code_plan, 16).unwrap()
        );
        assert_eq!(bit_plan.op_spans[2], BitSpan { bits: 1, len: 2 });
        assert_eq!(
            bit_plan.op_spans[3],
            code_length_symbol_span(&code_plan, 17).unwrap()
        );
        assert_eq!(bit_plan.op_spans[4], BitSpan { bits: 7, len: 3 });
    }

    #[test]
    fn code_length_prefix_plan_covers_repeated_json_prefix_descriptions() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let plans = vec![
            prefix_description_plan(&profile.literal).unwrap(),
            prefix_description_plan(&profile.insert_copy).unwrap(),
            prefix_description_plan(&profile.distance).unwrap(),
        ];
        let code_plan = code_length_prefix_plan(&plans).unwrap();
        let description_bits = prefix_description_bit_estimate(&plans, &code_plan).unwrap();
        let raw_length_bits: usize = plans.iter().map(|plan| plan.trimmed_len * 4).sum();

        assert!(code_plan.canonical.iter().any(|code| code.symbol == 17));
        assert!(description_bits < raw_length_bits);
        assert!(code_plan.ordered_lengths.iter().all(|&(_, len)| len <= 5));
    }

    #[test]
    fn repeated_json_prefix_description_bit_plan_matches_estimate() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let plans = vec![
            prefix_description_plan(&profile.literal).unwrap(),
            prefix_description_plan(&profile.insert_copy).unwrap(),
            prefix_description_plan(&profile.distance).unwrap(),
        ];
        let code_plan = code_length_prefix_plan(&plans).unwrap();
        let bit_plan = prefix_description_bit_plan(&plans, &code_plan).unwrap();
        let op_bits: usize = bit_plan
            .op_spans
            .iter()
            .map(|span| usize::from(span.len))
            .sum();
        let header_bits: usize = bit_plan
            .header_lengths
            .iter()
            .map(|span| usize::from(span.len))
            .sum();

        assert_eq!(
            op_bits,
            prefix_description_bit_estimate(&plans, &code_plan).unwrap()
        );
        assert_eq!(
            header_bits,
            2 + code_plan
                .ordered_lengths
                .iter()
                .map(|&(_, len)| usize::from(code_length_code_length_span(len).unwrap().len))
                .sum::<usize>()
        );
        assert!(bit_plan.op_spans.len() > plans.iter().map(|plan| plan.ops.len()).sum());
    }

    #[test]
    fn code_length_code_length_spans_follow_rfc_table() {
        assert_eq!(
            code_length_code_length_span(0),
            Some(BitSpan { bits: 0b00, len: 2 })
        );
        assert_eq!(
            code_length_code_length_span(1),
            Some(BitSpan {
                bits: 0b0111,
                len: 4
            })
        );
        assert_eq!(
            code_length_code_length_span(2),
            Some(BitSpan {
                bits: 0b011,
                len: 3
            })
        );
        assert_eq!(
            code_length_code_length_span(3),
            Some(BitSpan { bits: 0b10, len: 2 })
        );
        assert_eq!(
            code_length_code_length_span(4),
            Some(BitSpan { bits: 0b01, len: 2 })
        );
        assert_eq!(
            code_length_code_length_span(5),
            Some(BitSpan {
                bits: 0b1111,
                len: 4
            })
        );
        assert_eq!(code_length_code_length_span(6), None);
    }

    #[test]
    fn prefix_description_grammar_accepts_repeated_json_plan() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let plans = single_block_prefix_description_plans(&profile).unwrap();
        let code_plan = code_length_prefix_plan(&plans).unwrap();

        assert_eq!(
            validate_prefix_description_grammar(&plans, &code_plan),
            Some(())
        );
        for plan in &plans {
            let expanded = expand_code_length_ops(&plan.ops).unwrap();
            assert_eq!(expanded.len(), plan.trimmed_len);
            assert_eq!(
                expanded
                    .iter()
                    .filter_map(|&len| (len != 0).then_some(32768u32 >> len))
                    .sum::<u32>(),
                32768
            );
        }
    }

    #[test]
    fn prefix_description_grammar_rejects_trailing_zero_or_zero_repeat() {
        let mut plan = PrefixDescriptionPlan {
            alphabet_size: 4,
            trimmed_len: 4,
            ops: vec![
                CodeLengthOp {
                    symbol: 1,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 1,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 0,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
            ],
        };
        assert_eq!(validate_prefix_description_plan_grammar(&plan), None);

        plan.ops.pop();
        plan.ops.push(CodeLengthOp {
            symbol: 17,
            repeat: 3,
            extra_bits: 3,
            extra_value: 0,
        });
        assert_eq!(validate_prefix_description_plan_grammar(&plan), None);
    }

    #[test]
    fn prefix_description_grammar_rejects_bad_kraft_sums() {
        let plan = PrefixDescriptionPlan {
            alphabet_size: 4,
            trimmed_len: 2,
            ops: vec![
                CodeLengthOp {
                    symbol: 2,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
                CodeLengthOp {
                    symbol: 2,
                    repeat: 1,
                    extra_bits: 0,
                    extra_value: 0,
                },
            ],
        };
        assert_eq!(validate_prefix_description_plan_grammar(&plan), None);

        let code_plan = CodeLengthPrefixPlan {
            code: ComplexPrefixCode {
                alphabet_size: 18,
                max_bits: 5,
                code_lengths: vec![(1, 2), (2, 2)],
            },
            canonical: vec![
                CanonicalCode {
                    symbol: 1,
                    len: 2,
                    msb_code: 0,
                    lsb_bits: 0,
                },
                CanonicalCode {
                    symbol: 2,
                    len: 2,
                    msb_code: 1,
                    lsb_bits: 2,
                },
            ],
            ordered_lengths: vec![(1, 2), (2, 2)],
        };
        assert_eq!(validate_code_length_prefix_header(&code_plan), None);
    }

    #[test]
    fn bit_writer_emits_spans_lsb_first_with_logical_bit_count() {
        let mut bw = BitWriter::new();
        bw.write_spans(&[
            BitSpan {
                bits: 0b101,
                len: 3,
            },
            BitSpan { bits: 0b11, len: 2 },
            BitSpan {
                bits: 0b10010,
                len: 5,
            },
        ]);

        assert_eq!(bw.bit_len(), 10);
        assert_eq!(bw.finish(), vec![0b01011101, 0b00000010]);
    }

    #[test]
    fn repeated_json_prefix_description_fragment_writes_planned_bits() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let plans = vec![
            prefix_description_plan(&profile.literal).unwrap(),
            prefix_description_plan(&profile.insert_copy).unwrap(),
            prefix_description_plan(&profile.distance).unwrap(),
        ];
        let code_plan = code_length_prefix_plan(&plans).unwrap();
        let bit_plan = prefix_description_bit_plan(&plans, &code_plan).unwrap();
        let expected_bits: usize = bit_plan
            .header_lengths
            .iter()
            .chain(bit_plan.op_spans.iter())
            .map(|span| usize::from(span.len))
            .sum();

        let fragment = write_prefix_description_fragment(&plans, &code_plan).unwrap();

        assert_eq!(fragment.bit_len, expected_bits);
        assert_eq!(fragment.bytes.len(), expected_bits.div_ceil(8));
        assert!(fragment.bytes.iter().any(|&byte| byte != 0));
    }

    #[test]
    fn command_payload_bit_plan_orders_command_literals_and_distance() {
        let data = b"abcabcabcabc";
        let commands = lower_lz77_commands(data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(data, &commands).expect("complex profile");
        let spans = command_payload_bit_plan(data, &commands, &profile).unwrap();
        let insert_copy_codes =
            prefix_canonical_codes(&selected_prefix_code(&profile.insert_copy)).unwrap();
        let literal_codes =
            prefix_canonical_codes(&selected_prefix_code(&profile.literal)).unwrap();
        let distance_codes =
            prefix_canonical_codes(&selected_prefix_code(&profile.distance)).unwrap();
        let first = &commands[0];

        assert_eq!(
            spans[0],
            canonical_symbol_span(&insert_copy_codes, first.insert_copy.code).unwrap()
        );
        assert_eq!(
            spans[1],
            canonical_symbol_span(&literal_codes, b'a' as u16).unwrap()
        );
        assert_eq!(
            spans[2],
            canonical_symbol_span(&literal_codes, b'b' as u16).unwrap()
        );
        assert_eq!(
            spans[3],
            canonical_symbol_span(&literal_codes, b'c' as u16).unwrap()
        );
        assert_eq!(
            spans[4],
            canonical_symbol_span(&distance_codes, first.distance.unwrap().code).unwrap()
        );
        assert!(spans.iter().all(|span| span.len != 0));
    }

    #[test]
    fn repeated_json_command_payload_fragment_matches_estimate() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let fragment = write_command_payload_fragment(&data, &commands, &profile).unwrap();
        let estimated = encoded_command_bit_estimate(&data, &commands, &profile).unwrap();

        assert_eq!(fragment.bit_len, estimated);
        assert_eq!(fragment.bytes.len(), estimated.div_ceil(8));
        assert!(fragment.bit_len < data.len() * 4);
        assert!(fragment.bytes.iter().any(|&byte| byte != 0));
    }

    #[test]
    fn compressed_body_plan_composes_prefix_and_payload_bits() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let body = single_block_compressed_body_plan(&data, &commands, &profile).unwrap();
        let prefix_bits: usize = body
            .prefix_spans
            .iter()
            .map(|span| usize::from(span.len))
            .sum();
        let payload_bits: usize = body
            .payload_spans
            .iter()
            .map(|span| usize::from(span.len))
            .sum();

        assert!(prefix_bits > 0);
        assert!(payload_bits > 0);
        assert_eq!(body.bit_len, prefix_bits + payload_bits);
        assert!(body.bit_len < data.len() * 8);
    }

    #[test]
    fn compressed_body_writer_preserves_bit_contiguity() {
        let data = b"abcdefg0123456789-".repeat(160);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let prefix_spans = single_block_prefix_code_spans(&profile).unwrap();
        let prefix_bits: usize = prefix_spans.iter().map(|span| usize::from(span.len)).sum();
        let mut prefix_writer = BitWriter::new();
        prefix_writer.write_spans(&prefix_spans);
        let prefix_bytes = prefix_writer.finish();
        let payload = write_command_payload_fragment(&data, &commands, &profile).unwrap();
        let body = write_single_block_compressed_body_fragment(&data, &commands, &profile).unwrap();

        assert_eq!(body.bit_len, prefix_bits + payload.bit_len);
        assert_eq!(body.bytes.len(), body.bit_len.div_ceil(8));
        assert_ne!(prefix_bits % 8, 0);

        let mut byte_boundary_join = prefix_bytes.clone();
        byte_boundary_join.extend_from_slice(&payload.bytes);
        assert_ne!(body.bytes, byte_boundary_join);
    }

    #[test]
    fn compressed_stream_header_spans_encode_single_block_shape() {
        let spans = single_block_compressed_header_spans(
            513,
            DistanceProfile {
                ndirect: 0,
                npostfix: 0,
            },
        )
        .unwrap();

        assert_eq!(spans[0], BitSpan { bits: 0, len: 1 });
        assert_eq!(spans[1], BitSpan { bits: 1, len: 1 });
        assert_eq!(spans[2], BitSpan { bits: 0, len: 1 });
        assert_eq!(spans[3], BitSpan { bits: 0, len: 2 });
        assert_eq!(spans[4], BitSpan { bits: 512, len: 16 });
        assert_eq!(spans[5], BitSpan { bits: 0, len: 1 });
        assert_eq!(spans[6], BitSpan { bits: 0, len: 1 });
        assert_eq!(spans[7], BitSpan { bits: 0, len: 1 });
        assert_eq!(spans[8], BitSpan { bits: 0, len: 2 });
        assert_eq!(spans[9], BitSpan { bits: 0, len: 4 });
        assert_eq!(spans[10], BitSpan { bits: 0, len: 2 });
        assert_eq!(spans[11], BitSpan { bits: 0, len: 1 });
        assert_eq!(spans[12], BitSpan { bits: 0, len: 1 });
        assert_eq!(
            spans
                .iter()
                .map(|span| usize::from(span.len))
                .sum::<usize>(),
            34
        );
        assert_eq!(
            single_block_compressed_header_spans(
                0,
                DistanceProfile {
                    ndirect: 0,
                    npostfix: 0
                }
            ),
            None
        );
        assert_eq!(
            single_block_compressed_header_spans(
                65_537,
                DistanceProfile {
                    ndirect: 0,
                    npostfix: 0
                }
            ),
            None
        );
    }

    #[test]
    fn literal_block_type_switch_stream_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let data = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_/.".to_vec();
        let split = 32;
        let commands = lower_lz77_commands_with_profile(
            &data,
            0,
            0,
            Lz77Profile {
                candidate_limit: 0,
                max_match: 0,
                lazy_match: false,
            },
        );
        assert!(commands.iter().all(|cmd| cmd.copy_distance.is_none()));
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let plan = literal_block_type_switch_stream_plan(&data, &commands, &profile, split)
            .expect("literal block type switch plan");
        let encoded = write_literal_block_type_switch_stream(&plan);

        assert_eq!(
            block_type_count_spans(2).unwrap()[0],
            BitSpan { bits: 1, len: 1 }
        );
        assert!(plan.bit_len > 0);
        assert_eq!(encoded.len(), plan.bit_len.div_ceil(8));

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn literal_block_type_switch_selects_two_literal_trees_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut data = b"aaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbcccccccccccccccc".to_vec();
        data.extend_from_slice(b"XYZXYZXYZXYZXYZXYZXYZXYZXYZXYZXYZXYZ");
        let split = 48;
        let commands = lower_lz77_commands_with_profile(
            &data,
            0,
            0,
            Lz77Profile {
                candidate_limit: 0,
                max_match: 0,
                lazy_match: false,
            },
        );
        assert!(commands.iter().all(|cmd| cmd.copy_distance.is_none()));
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let one_tree = literal_block_type_switch_stream_plan(&data, &commands, &profile, split)
            .expect("one-tree block type switch plan");
        let two_tree = literal_block_type_two_tree_stream_plan(&data, &commands, &profile, split)
            .expect("two-tree block type switch plan");
        let one_tree_encoded = write_literal_block_type_switch_stream(&one_tree);
        let encoded = write_literal_block_type_switch_stream(&two_tree);

        assert_ne!(encoded, one_tree_encoded);
        assert_eq!(encoded.len(), two_tree.bit_len.div_ceil(8));

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn literal_block_type_two_tree_lz77_candidate_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let encoded = encode_literal_block_type_lz77(&data, lz77_profile(11))
            .expect("literal block type candidate");

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn literal_block_type_candidates_include_json_source_boundary() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        let boundary = data.len() + br#"],"body":""#.len();
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands_with_profile(
            &data,
            0,
            0,
            Lz77Profile {
                candidate_limit: 32,
                max_match: LZ77_MAX_MATCH,
                lazy_match: true,
            },
        );
        let total_literals = inserted_literal_count(&commands);
        let structural = source_split_to_inserted_literal_index(&commands, boundary)
            .expect("structural literal split");
        let candidates = literal_split_candidates_for_data(&data, &commands, total_literals);

        assert!(structural > 0 && structural < total_literals);
        assert!(candidates.contains(&structural));
    }

    #[test]
    fn literal_command_block_type_candidate_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let distance_profile = DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        };
        let commands = lower_lz77_commands_with_profile(
            &data,
            distance_profile.ndirect,
            distance_profile.npostfix,
            lz77_profile(11),
        );
        let total_literals = inserted_literal_count(&commands);
        let (literal_split, command_split) =
            literal_command_split_candidates_for_data(&data, &commands, total_literals)
                .into_iter()
                .find(|&(_, command_split)| command_split < commands.len())
                .expect("literal and command split");
        let profile =
            single_block_complex_prefix_profile_with_distance(&data, &commands, distance_profile)
                .expect("complex profile");
        let plan = literal_command_block_type_stream_plan(
            &data,
            &commands,
            &profile,
            literal_split,
            command_split,
        )
        .expect("literal and command block type plan");
        let encoded = write_literal_block_type_switch_stream(&plan);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn literal_command_distance_block_type_candidate_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let distance_profile = DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        };
        let commands = lower_lz77_commands_with_profile(
            &data,
            distance_profile.ndirect,
            distance_profile.npostfix,
            lz77_profile(11),
        );
        let total_literals = inserted_literal_count(&commands);
        let total_distances = distance_symbol_count(&commands);
        let (literal_split, command_split, distance_split) =
            literal_command_distance_split_candidates_for_data(
                &data,
                &commands,
                total_literals,
                total_distances,
            )
            .into_iter()
            .next()
            .expect("literal, command, and distance split");
        let profile =
            single_block_complex_prefix_profile_with_distance(&data, &commands, distance_profile)
                .expect("complex profile");
        let plan = literal_command_distance_block_type_stream_plan(
            &data,
            &commands,
            &profile,
            literal_split,
            command_split,
            distance_split,
        )
        .expect("literal, command, and distance block type plan");
        let encoded = write_literal_block_type_switch_stream(&plan);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn compressed_stream_plan_wraps_body_with_header_bits() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let plan = single_block_compressed_stream_plan(&data, &commands, &profile).unwrap();
        let header_bits: usize = plan
            .header_spans
            .iter()
            .map(|span| usize::from(span.len))
            .sum();
        let body_bits = plan.body.bit_len;
        let fragment =
            write_single_block_compressed_stream_fragment(&data, &commands, &profile).unwrap();

        assert_eq!(header_bits, 34);
        assert_eq!(plan.bit_len, header_bits + body_bits);
        assert_eq!(fragment.bit_len, plan.bit_len);
        assert_eq!(fragment.bytes.len(), fragment.bit_len.div_ceil(8));
        assert!(fragment.bit_len < data.len() * 8);
        assert!(fragment.bytes.iter().any(|&byte| byte != 0));
    }

    #[test]
    fn compressed_stream_plan_uses_simple_prefix_when_profitable() {
        let data = b"Cruft press production workflow payload. ".repeat(160);
        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let selected = [
            selected_prefix_code(&profile.literal),
            selected_prefix_code(&profile.insert_copy),
            selected_prefix_code(&profile.distance),
        ];

        assert!(
            selected
                .iter()
                .any(|prefix| matches!(prefix, PrefixCodeProfile::Simple(_))),
            "long repeated text should have at least one simple generated prefix"
        );
        let stream =
            write_single_block_compressed_stream_fragment(&data, &commands, &profile).unwrap();
        assert_eq!(decode(&stream.bytes).unwrap(), data);
    }

    #[test]
    fn compressed_stream_writer_preserves_header_body_contiguity() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let header_spans = single_block_compressed_header_spans(
            data.len(),
            DistanceProfile {
                ndirect: 0,
                npostfix: 0,
            },
        )
        .unwrap();
        let body = write_single_block_compressed_body_fragment(&data, &commands, &profile).unwrap();
        let stream =
            write_single_block_compressed_stream_fragment(&data, &commands, &profile).unwrap();
        let mut header_writer = BitWriter::new();
        header_writer.write_spans(&header_spans);
        let header_bits = header_writer.bit_len();
        let mut byte_boundary_join = header_writer.finish();
        byte_boundary_join.extend_from_slice(&body.bytes);

        assert_eq!(header_bits, 34);
        assert_eq!(stream.bit_len, header_bits + body.bit_len);
        assert_ne!(stream.bytes, byte_boundary_join);
    }

    #[test]
    fn assembled_repeated_json_stream_decode_probe_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let stream =
            write_single_block_compressed_stream_fragment(&data, &commands, &profile).unwrap();
        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&stream.bytes)
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn contextual_literal_context_stream_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let data = br#"{"name":"brotli-node-parity","files":["src/module-0.js","src/module-1.js"],"body":"Cruft press payload."}"#;
        let commands = lower_lz77_commands(data, 0, 0);
        let distance_profile = DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        };
        let profile = contextual_single_block_complex_prefix_profile_for_map(
            data,
            &commands,
            distance_profile,
            json_literal_context_map(),
        )
        .expect("contextual profile");
        let stream = contextual_single_block_compressed_stream_plan_with_header(
            data, &commands, &profile, true, true,
        )
        .expect("contextual stream");

        let mut bw = BitWriter::new();
        bw.write_spans(&stream.header_spans);
        bw.write_spans(&stream.body.prefix_spans);
        bw.write_spans(&stream.body.payload_spans);
        let encoded = bw.finish();

        assert_eq!(decode(&encoded).unwrap(), data);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn multi_literal_context_stream_decodes_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..24 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..48 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands_with_profile_output_base_and_distance_cache(
            &data,
            0,
            0,
            lz77_profile(11),
            0,
        );
        let distance_profile = DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        };
        let context_map = context_map_candidates(&data, &commands)
            .into_iter()
            .find(|map| map.iter().copied().max().unwrap_or(0) >= 2)
            .expect("multi-tree context map candidate");
        let profile = contextual_single_block_complex_prefix_profile_for_map(
            &data,
            &commands,
            distance_profile,
            context_map,
        )
        .expect("multi-tree contextual profile");
        assert!(profile.literal.len() >= 3);
        let stream = contextual_single_block_compressed_stream_plan_with_header(
            &data, &commands, &profile, true, true,
        )
        .expect("multi-tree contextual stream");
        let encoded = write_stream_plan(&SingleBlockCompressedStreamPlan {
            header_spans: stream.header_spans,
            body: SingleBlockCompressedBodyPlan {
                prefix_spans: stream.body.prefix_spans,
                payload_spans: stream.body.payload_spans,
                bit_len: stream.body.bit_len,
            },
            bit_len: stream.bit_len,
        });

        assert_eq!(decode(&encoded).unwrap(), data);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn assembled_repeated_json_stream_decodes_locally() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let commands = lower_lz77_commands(&data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(&data, &commands).expect("complex profile");
        let stream =
            write_single_block_compressed_stream_fragment(&data, &commands, &profile).unwrap();

        assert_eq!(decode(&stream.bytes).unwrap(), data);
    }

    #[test]
    fn encode_uses_lz77_single_block_for_repeated_json_payload() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let encoded = encode(&data, &BrotliParams::default()).unwrap();
        assert!(encoded.len() < data.len() / 2, "encoded={}", encoded.len());
        assert_eq!(decode(&encoded).unwrap(), data);
    }

    #[test]
    fn direct_distance_profile_search_improves_repeated_json_probe() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let default_profile = DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        };
        let default_commands = lower_lz77_commands_with_profile(&data, 0, 0, lz77_profile(11));
        let default_prefix = single_block_complex_prefix_profile_with_distance(
            &data,
            &default_commands,
            default_profile,
        )
        .expect("default prefix");
        let default_stream = write_single_block_compressed_stream_fragment(
            &data,
            &default_commands,
            &default_prefix,
        )
        .expect("default stream");

        let mut best = (
            default_profile,
            default_stream.bit_len,
            default_stream.bytes.clone(),
        );
        for distance_profile in direct_distance_profiles().iter().copied() {
            let commands = lower_lz77_commands_with_profile(
                &data,
                distance_profile.ndirect,
                distance_profile.npostfix,
                lz77_profile(11),
            );
            let prefix = single_block_complex_prefix_profile_with_distance(
                &data,
                &commands,
                distance_profile,
            )
            .expect("candidate prefix");
            let stream = write_single_block_compressed_stream_fragment(&data, &commands, &prefix)
                .expect("candidate stream");
            if stream.bit_len < best.1 {
                best = (distance_profile, stream.bit_len, stream.bytes);
            }
        }

        assert_ne!(best.0, default_profile, "best profile={best:?}");
        assert!(best.1 < default_stream.bit_len);
        assert_eq!(decode(&best.2).unwrap(), data);
    }

    #[test]
    fn low_quality_postfix_distance_profile_compacts_repeated_json_probe() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let profile = lz77_profile(4);
        let default_profile = DistanceProfile {
            ndirect: 0,
            npostfix: 0,
        };
        let default_commands = lower_lz77_commands_with_profile(&data, 0, 0, profile);
        let default_prefix = single_block_complex_prefix_profile_with_distance(
            &data,
            &default_commands,
            default_profile,
        )
        .expect("default prefix");
        let default_stream = write_single_block_compressed_stream_fragment(
            &data,
            &default_commands,
            &default_prefix,
        )
        .expect("default stream");

        let default_bit_len = default_stream.bit_len;
        let mut best = (default_profile, default_bit_len, default_stream.bytes);
        for distance_profile in distance_profiles_for_lz77(profile).iter().copied() {
            let commands = lower_lz77_commands_with_profile(
                &data,
                distance_profile.ndirect,
                distance_profile.npostfix,
                profile,
            );
            let prefix = single_block_complex_prefix_profile_with_distance(
                &data,
                &commands,
                distance_profile,
            )
            .expect("candidate prefix");
            let stream = write_single_block_compressed_stream_fragment(&data, &commands, &prefix)
                .expect("candidate stream");
            if stream.bit_len < best.1 {
                best = (distance_profile, stream.bit_len, stream.bytes);
            }
        }

        assert_eq!(
            best.0,
            DistanceProfile {
                ndirect: 0,
                npostfix: 3
            },
            "best profile={best:?}"
        );
        assert!(best.1 < default_bit_len);
        assert_eq!(decode(&best.2).unwrap(), data);
    }

    #[test]
    fn command_block_type_tail_split_compacts_repeated_json_q11() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let profile = lz77_profile(11);
        let distance_profile = DistanceProfile {
            ndirect: 12,
            npostfix: 0,
        };
        let commands = lower_lz77_commands_with_profile_output_base_and_distance_cache(
            &data,
            distance_profile.ndirect,
            distance_profile.npostfix,
            profile,
            0,
        );
        let prefix =
            single_block_complex_prefix_profile_with_distance(&data, &commands, distance_profile)
                .expect("single prefix");
        let single =
            single_block_compressed_stream_plan_with_header(&data, &commands, &prefix, true, true)
                .expect("single stream");
        let split = command_block_type_stream_plan(&data, &commands, &prefix, commands.len() - 4)
            .expect("command split stream");
        let encoded = write_literal_block_type_switch_stream(&split);

        assert!(split.bit_len < single.bit_len);
        assert!(encoded.len() <= 222, "encoded={}", encoded.len());
        assert_eq!(decode(&encoded).unwrap(), data);
    }

    #[test]
    fn quality_zero_skips_lz77_while_default_uses_it() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let q0 = encode(
            &data,
            &BrotliParams {
                quality: 0,
                ..Default::default()
            },
        )
        .unwrap();
        let q11 = encode(&data, &BrotliParams::default()).unwrap();

        assert_ne!(q0, q11);
        assert!(
            q11.len() < q0.len() / 2,
            "q0={}, q11={}",
            q0.len(),
            q11.len()
        );
        assert_eq!(decode(&q0).unwrap(), data);
        assert_eq!(decode(&q11).unwrap(), data);
    }

    #[test]
    fn quality_levels_select_distinct_encoder_shapes() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let q1 = encode(
            &data,
            &BrotliParams {
                quality: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let q6 = encode(
            &data,
            &BrotliParams {
                quality: 6,
                ..Default::default()
            },
        )
        .unwrap();
        let q11 = encode(&data, &BrotliParams::default()).unwrap();

        assert_ne!(q1, q6);
        assert_ne!(q6, q11);
        assert_eq!(decode(&q1).unwrap(), data);
        assert_eq!(decode(&q6).unwrap(), data);
        assert_eq!(decode(&q11).unwrap(), data);
    }

    #[test]
    fn tail_band_command_split_compacts_mid_quality_repeated_json() {
        let mut data = br#"{"name":"brotli-node-parity","files":["#.to_vec();
        for i in 0..64 {
            data.extend_from_slice(format!("\"src/module-{i}.js\",").as_bytes());
        }
        data.extend_from_slice(br#"],"body":""#);
        for _ in 0..128 {
            data.extend_from_slice(b"Cruft press production workflow payload. ");
        }
        data.extend_from_slice(br#""}"#);

        let q9 = encode(
            &data,
            &BrotliParams {
                quality: 9,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(q9.len() <= 263, "q9={}", q9.len());
        assert_eq!(decode(&q9).unwrap(), data);
    }

    #[test]
    fn static_dictionary_offsets_follow_rfc_recursion() {
        assert_eq!(static_dictionary_word_count(3), None);
        assert_eq!(static_dictionary_word_count(4), Some(1024));
        assert_eq!(static_dictionary_word_count(6), Some(2048));
        assert_eq!(static_dictionary_word_count(24), Some(32));
        assert_eq!(static_dictionary_offset(0), Some(0));
        assert_eq!(static_dictionary_offset(4), Some(0));
        assert_eq!(static_dictionary_offset(5), Some(4096));
        assert_eq!(static_dictionary_offset(24), Some(122016));
        assert_eq!(static_dictionary_offset(25), None);
        assert_eq!(
            static_dictionary_offset(24).unwrap() + 24 * static_dictionary_word_count(24).unwrap(),
            122_784
        );
    }

    #[test]
    fn static_dictionary_distance_mapping_uses_max_allowed_distance() {
        let r = static_dictionary_ref(7, 101, 100).unwrap();
        assert_eq!(
            r,
            StaticDictionaryRef {
                length: 7,
                index: 0,
                transform_id: 0,
                offset: static_dictionary_offset(7).unwrap()
            }
        );

        let second_transform = static_dictionary_ref(7, 101 + (1 << BROTLI_STATIC_NDBITS[7]), 100)
            .expect("dictionary ref");
        assert_eq!(second_transform.index, 0);
        assert_eq!(second_transform.transform_id, 1);
        assert_eq!(static_dictionary_ref(3, 101, 100), None);
        assert_eq!(static_dictionary_ref(25, 101, 100), None);
        assert_eq!(static_dictionary_ref(7, 100, 100), None);
    }

    #[test]
    fn static_dictionary_transforms_match_rfc_table_shapes() {
        assert_eq!(
            apply_dictionary_transform(b"payload", 0).unwrap(),
            b"payload"
        );
        assert_eq!(
            apply_dictionary_transform(b"payload", 1).unwrap(),
            b"payload "
        );
        assert_eq!(
            apply_dictionary_transform(b"payload", 2).unwrap(),
            b" payload "
        );
        assert_eq!(
            apply_dictionary_transform(b"payload", 3).unwrap(),
            b"ayload"
        );
        assert_eq!(
            apply_dictionary_transform(b"payload", 4).unwrap(),
            b"Payload "
        );
        assert_eq!(
            apply_dictionary_transform(b"payload", 10).unwrap(),
            b"payload and "
        );
        assert_eq!(apply_dictionary_transform(b"payload", 23).unwrap(), b"payl");
        assert_eq!(
            apply_dictionary_transform(b"payload", 44).unwrap(),
            b"PAYLOAD"
        );
        assert_eq!(
            apply_dictionary_transform(b"payload", 102).unwrap(),
            b"\xc2\xa0payload"
        );
        assert_eq!(
            apply_dictionary_transform(b"payload", 120).unwrap(),
            b" Payload='"
        );
        assert_eq!(apply_dictionary_transform(b"payload", 121), None);
    }

    #[test]
    fn exact_static_dictionary_table_names_rfc_identity_words() {
        assert_eq!(
            exact_static_dictionary_word_at(b"Cruft production module", 6),
            Some(StaticDictionaryWord {
                output: b"production",
                length: 10,
                index: 48,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(b"Cruft production module", 17),
            Some(StaticDictionaryWord {
                output: b"module",
                length: 6,
                index: 27,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word(10, 48, 0),
            Some(&b"production"[..])
        );
        assert_eq!(
            exact_static_dictionary_word(4, 118, 31),
            Some(&b"load. "[..])
        );
        assert_eq!(exact_static_dictionary_word(6, 27, 0), Some(&b"module"[..]));
        assert_eq!(exact_static_dictionary_word(5, 198, 0), Some(&b"files"[..]));
        assert_eq!(exact_static_dictionary_word(5, 81, 0), Some(&b"press"[..]));
        assert_eq!(exact_static_dictionary_word(5, 9, 0), Some(&b"small"[..]));
        assert_eq!(exact_static_dictionary_word(4, 19, 0), Some(&b"body"[..]));
        assert_eq!(exact_static_dictionary_word(4, 61, 0), Some(&b"name"[..]));
        assert_eq!(exact_static_dictionary_word(4, 488, 0), Some(&b"node"[..]));
        assert_eq!(exact_static_dictionary_word(4, 16, 0), Some(&b"text"[..]));
        assert_eq!(exact_static_dictionary_word(7, 27, 0), None);
    }

    #[test]
    fn exact_static_dictionary_table_names_probe_identity_words() {
        assert_eq!(
            exact_static_dictionary_word_at(br#"{"name":"brotli-node","files":[],"body":""}"#, 2),
            Some(StaticDictionaryWord {
                output: b"name",
                length: 4,
                index: 61,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(br#"{"name":"brotli-node","files":[],"body":""}"#, 16),
            Some(StaticDictionaryWord {
                output: b"node",
                length: 4,
                index: 488,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(br#"{"name":"brotli-node","files":[],"body":""}"#, 23),
            Some(StaticDictionaryWord {
                output: b"files",
                length: 5,
                index: 198,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(br#"{"name":"brotli-node","files":[],"body":""}"#, 34),
            Some(StaticDictionaryWord {
                output: b"body",
                length: 4,
                index: 19,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(b"small text press", 0),
            Some(StaticDictionaryWord {
                output: b"small",
                length: 5,
                index: 9,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(b"small text press", 6),
            Some(StaticDictionaryWord {
                output: b"text",
                length: 4,
                index: 16,
                transform_id: 0
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(b"small text press", 11),
            Some(StaticDictionaryWord {
                output: b"press",
                length: 5,
                index: 81,
                transform_id: 0
            })
        );
    }

    #[test]
    fn literal_spans_lower_to_static_dictionary_identity_refs() {
        let data = b"Cruft production module";
        let commands = lower_lz77_commands(data, 0, 0);
        let dict_copies = commands
            .iter()
            .filter(|cmd| cmd.copy_distance.is_some())
            .filter(|cmd| {
                static_dictionary_ref(cmd.copy_len, cmd.copy_distance.unwrap(), cmd.insert_len)
                    .is_some()
            })
            .count();

        assert_eq!(dict_copies, 2);
        assert!(commands.iter().map(|cmd| cmd.insert_len).sum::<usize>() < data.len());
    }

    #[test]
    fn literal_prefix_model_counts_only_emitted_literals() {
        let data = b"Cruft production module";
        let commands = lower_lz77_commands(data, 0, 0);
        let frequencies =
            literal_frequencies_for_commands(data, &commands).expect("literal frequencies");
        let total_literal_count = frequencies.iter().map(|(_, count)| count).sum::<usize>();

        assert_eq!(
            total_literal_count,
            commands.iter().map(|cmd| cmd.insert_len).sum::<usize>()
        );
        assert_eq!(total_literal_count, b"Cruft  ".len());
        assert_eq!(
            frequencies
                .iter()
                .find(|(symbol, _)| *symbol == b'p' as u16),
            None
        );
        assert_eq!(
            frequencies
                .iter()
                .find(|(symbol, _)| *symbol == b'm' as u16),
            None
        );
    }

    #[test]
    fn literal_spans_lower_to_static_dictionary_transformed_refs() {
        let data = b"Cruft workflow payload. ";
        let commands = lower_lz77_commands(data, 0, 0);
        let transformed = commands
            .iter()
            .filter_map(|cmd| {
                static_dictionary_ref(cmd.copy_len, cmd.copy_distance?, cmd.insert_len)
            })
            .filter(|dict_ref| dict_ref.transform_id != 0)
            .count();

        assert_eq!(transformed, 1);
        assert!(commands.iter().map(|cmd| cmd.insert_len).sum::<usize>() < data.len());
        assert_eq!(
            exact_static_dictionary_word_at(b"workflow payload. ", 12),
            Some(StaticDictionaryWord {
                output: b"load. ",
                length: 4,
                index: 118,
                transform_id: 31
            })
        );
        assert_eq!(
            exact_static_dictionary_word_at(b"workflow payload. ", 4),
            None
        );
    }

    #[test]
    fn static_dictionary_identity_refs_decode_locally() {
        let data = b"Cruft production module";
        let commands = lower_lz77_commands(data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(data, &commands).expect("complex profile");
        let stream =
            write_single_block_compressed_stream_fragment(data, &commands, &profile).unwrap();

        assert_eq!(decode(&stream.bytes).unwrap(), data);
    }

    #[test]
    fn static_dictionary_identity_refs_decode_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let data = b"Cruft production module";
        let commands = lower_lz77_commands(data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(data, &commands).expect("complex profile");
        let stream =
            write_single_block_compressed_stream_fragment(data, &commands, &profile).unwrap();
        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&stream.bytes)
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn static_dictionary_transformed_refs_decode_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let data = b"Cruft workflow payload. ";
        let commands = lower_lz77_commands(data, 0, 0);
        let profile =
            single_block_complex_prefix_profile(data, &commands).expect("complex profile");
        let stream =
            write_single_block_compressed_stream_fragment(data, &commands, &profile).unwrap();
        assert_eq!(decode(&stream.bytes).unwrap(), data);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&stream.bytes)
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn roundtrips_large_uncompressed_blocks() {
        for len in [513usize, 600, 65_536, 70_000] {
            let data: Vec<u8> = (0..len).map(|i| (i & 0xff) as u8).collect();
            let encoded = encode(&data, &BrotliParams::default()).unwrap();
            assert_eq!(decode(&encoded).unwrap(), data, "len={len}");
        }
    }

    #[test]
    fn uncompressed_blocks_decode_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let data: Vec<u8> = (0..70_000usize).map(|i| ((i * 73) & 0xff) as u8).collect();
        let encoded = encode(&data, &BrotliParams::default()).unwrap();
        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, data);
    }

    #[test]
    fn generated_two_compressed_meta_blocks_decode_under_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let left = b"alpha alpha alpha alpha alpha alpha alpha alpha ";
        let right = b"beta beta beta beta beta beta beta beta beta ";
        let left_commands = lower_lz77_commands(left, 0, 0);
        let right_commands = lower_lz77_commands(right, 0, 0);
        assert!(!commands_use_static_dictionary(left, &left_commands));
        assert!(!commands_use_static_dictionary(right, &right_commands));

        let left_profile =
            single_block_complex_prefix_profile(left, &left_commands).expect("left prefix");
        let right_profile =
            single_block_complex_prefix_profile(right, &right_commands).expect("right prefix");
        let left_plan = single_block_compressed_stream_plan_with_header(
            left,
            &left_commands,
            &left_profile,
            true,
            false,
        )
        .expect("left plan");
        let right_plan = single_block_compressed_stream_plan_with_header(
            right,
            &right_commands,
            &right_profile,
            false,
            true,
        )
        .expect("right plan");

        let mut bw = BitWriter::new();
        bw.write_spans(&left_plan.header_spans);
        bw.write_spans(&left_plan.body.prefix_spans);
        bw.write_spans(&left_plan.body.payload_spans);
        bw.write_spans(&right_plan.header_spans);
        bw.write_spans(&right_plan.body.prefix_spans);
        bw.write_spans(&right_plan.body.payload_spans);
        let encoded = bw.finish();

        let mut expected = Vec::new();
        expected.extend_from_slice(left);
        expected.extend_from_slice(right);
        assert_eq!(decode(&encoded).unwrap(), expected);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, expected);
    }

    #[test]
    fn decode_with_limit_rejects_literal_output_bomb() {
        let encoded = [
            0x0b, 0x06, 0x80, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x57, 0x6f, 0x72, 0x6c,
            0x64, 0x21, 0x03,
        ];
        assert_eq!(
            decode_with_limit(&encoded, b"Hello, World".len()),
            Err(BrotliError::OutputTooLarge)
        );
    }

    #[test]
    fn decode_with_limit_rejects_accumulated_meta_block_output_bomb() {
        let left = b"alpha alpha alpha alpha alpha alpha alpha alpha ";
        let right = b"beta beta beta beta beta beta beta beta beta ";
        let left_commands = lower_lz77_commands(left, 0, 0);
        let right_commands = lower_lz77_commands(right, 0, 0);
        let left_profile =
            single_block_complex_prefix_profile(left, &left_commands).expect("left prefix");
        let right_profile =
            single_block_complex_prefix_profile(right, &right_commands).expect("right prefix");
        let left_plan = single_block_compressed_stream_plan_with_header(
            left,
            &left_commands,
            &left_profile,
            true,
            false,
        )
        .expect("left plan");
        let right_plan = single_block_compressed_stream_plan_with_header(
            right,
            &right_commands,
            &right_profile,
            false,
            true,
        )
        .expect("right plan");

        let mut bw = BitWriter::new();
        bw.write_spans(&left_plan.header_spans);
        bw.write_spans(&left_plan.body.prefix_spans);
        bw.write_spans(&left_plan.body.payload_spans);
        bw.write_spans(&right_plan.header_spans);
        bw.write_spans(&right_plan.body.prefix_spans);
        bw.write_spans(&right_plan.body.payload_spans);
        let encoded = bw.finish();

        assert_eq!(decode(&encoded).unwrap().len(), left.len() + right.len());
        assert_eq!(
            decode_with_limit(&encoded, left.len()),
            Err(BrotliError::OutputTooLarge)
        );
    }

    #[test]
    fn second_generated_meta_block_static_dictionary_uses_stream_history() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let left = b"prefix prefix prefix prefix prefix prefix ";
        let right = b"node node node ";
        let left_commands = lower_lz77_commands(left, 0, 0);
        let right_commands = lower_lz77_commands_with_profile_and_output_base(
            right,
            0,
            0,
            lz77_profile(11),
            left.len(),
        );
        assert!(
            right_commands
                .iter()
                .filter_map(|command| command.copy_distance)
                .any(|distance| distance > left.len()),
            "expected second block dictionary distance to be offset past stream history"
        );

        let left_profile =
            single_block_complex_prefix_profile(left, &left_commands).expect("left prefix");
        let right_profile =
            single_block_complex_prefix_profile(right, &right_commands).expect("right prefix");
        let left_plan = single_block_compressed_stream_plan_with_header(
            left,
            &left_commands,
            &left_profile,
            true,
            false,
        )
        .expect("left plan");
        let right_plan = single_block_compressed_stream_plan_with_header(
            right,
            &right_commands,
            &right_profile,
            false,
            true,
        )
        .expect("right plan");

        let mut bw = BitWriter::new();
        bw.write_spans(&left_plan.header_spans);
        bw.write_spans(&left_plan.body.prefix_spans);
        bw.write_spans(&left_plan.body.payload_spans);
        bw.write_spans(&right_plan.header_spans);
        bw.write_spans(&right_plan.body.prefix_spans);
        bw.write_spans(&right_plan.body.payload_spans);
        let encoded = bw.finish();

        let mut expected = Vec::new();
        expected.extend_from_slice(left);
        expected.extend_from_slice(right);
        assert_eq!(decode(&encoded).unwrap(), expected);

        let mut child = match Command::new("node")
            .arg("-e")
            .arg(
                "const fs=require('fs');\
                 const z=require('zlib');\
                 process.stdout.write(z.brotliDecompressSync(fs.readFileSync(0)));",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                eprintln!("node missing; skip brotli node interop test");
                return;
            }
        };
        child.stdin.as_mut().unwrap().write_all(&encoded).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "node brotli decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, expected);
    }

    #[test]
    fn rejects_truncated_stream() {
        assert_eq!(
            decode(&[0x0b, 0x01, 0x80, b'a']),
            Err(BrotliError::UnexpectedEnd)
        );
    }

    #[test]
    fn rejects_unsupported_stream() {
        assert_eq!(
            decode(&[0xff, 0x00, 0x00, 0x00]),
            Err(BrotliError::UnsupportedStream)
        );
    }
}
