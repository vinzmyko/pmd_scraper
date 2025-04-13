pub struct AnimationInfo {
    pub id: u8,
    pub name: &'static str,
    pub source: &'static str,
}

// Animation mapping that maps anim_id to group_idx
pub const ANIMATION_INFO: &[AnimationInfo] = &[
    AnimationInfo {
        id: 0,
        name: "Walk",
        source: "monster",
    },
    AnimationInfo {
        id: 1,
        name: "Attack",
        source: "m_attack",
    },
    AnimationInfo {
        id: 2,
        name: "Special_1",
        source: "m_attack",
    },
    AnimationInfo {
        id: 3,
        name: "Special_2",
        source: "m_attack",
    },
    AnimationInfo {
        id: 4,
        name: "Special_3",
        source: "m_attack",
    },
    AnimationInfo {
        id: 5,
        name: "Sleep",
        source: "monster",
    },
    AnimationInfo {
        id: 6,
        name: "Hurt",
        source: "monster",
    },
    AnimationInfo {
        id: 7,
        name: "Idle",
        source: "monster",
    },
    AnimationInfo {
        id: 8,
        name: "Swing",
        source: "m_attack",
    },
    AnimationInfo {
        id: 9,
        name: "Double",
        source: "m_attack",
    },
    AnimationInfo {
        id: 10,
        name: "Hop",
        source: "m_attack",
    },
    AnimationInfo {
        id: 11,
        name: "Charge",
        source: "monster",
    },
    AnimationInfo {
        id: 12,
        name: "Rotate",
        source: "m_attack",
    },
];

impl AnimationInfo {
    pub fn find_by_id(id: u8) -> Option<&'static AnimationInfo> {
        ANIMATION_INFO.iter().find(|&info| info.id == id)
    }

    pub fn find_by_id_and_source(id: u8, source: &str) -> Option<&'static AnimationInfo> {
        ANIMATION_INFO
            .iter()
            .find(|&info| info.id == id && info.source == source)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    Walk = 0,
    Attack = 1,
    Special1 = 2,
    Special2 = 3,
    Special3 = 4,
    Sleep = 5,
    Hurt = 6,
    Idle = 7,
    Swing = 8,
    Double = 9,
    Hop = 10,
    Charge = 11,
    Rotate = 12,
    Unknown = 255,
}

impl From<u8> for AnimationType {
    fn from(value: u8) -> Self {
        match value {
            0 => AnimationType::Walk,
            1 => AnimationType::Attack,
            2 => AnimationType::Special1,
            3 => AnimationType::Special2,
            4 => AnimationType::Special3,
            5 => AnimationType::Sleep,
            6 => AnimationType::Hurt,
            7 => AnimationType::Idle,
            8 => AnimationType::Swing,
            9 => AnimationType::Double,
            10 => AnimationType::Hop,
            11 => AnimationType::Charge,
            12 => AnimationType::Rotate,
            _ => AnimationType::Unknown,
        }
    }
}

impl AnimationType {
    pub fn name(&self) -> &'static str {
        match self {
            AnimationType::Walk => "Walk",
            AnimationType::Attack => "Attack",
            AnimationType::Special1 => "Special1",
            AnimationType::Special2 => "Special2",
            AnimationType::Special3 => "Special3",
            AnimationType::Sleep => "Sleep",
            AnimationType::Hurt => "Hurt",
            AnimationType::Idle => "Idle",
            AnimationType::Swing => "Swing",
            AnimationType::Double => "Double",
            AnimationType::Hop => "Hop",
            AnimationType::Charge => "Charge",
            AnimationType::Rotate => "Rotate",
            AnimationType::Unknown => "Unknown",
        }
    }
}
