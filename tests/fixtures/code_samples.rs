//! Original snippets and independently specified token colors.

use ratatui::style::Color;

pub struct Sample {
    pub language: &'static str,
    pub code: &'static str,
    pub token: &'static str,
    pub color: Color,
}

pub const SAMPLES: &[Sample] = &[
    Sample {
        language: "c",
        code: include_str!("code/sample.c"),
        token: "return",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "cpp",
        code: include_str!("code/sample.cpp"),
        token: "class",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "rust",
        code: include_str!("code/sample.rs"),
        token: "fn",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "terraform",
        code: include_str!("code/sample.tf"),
        token: "resource",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "javascript",
        code: include_str!("code/sample.js"),
        token: "const",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "typescript",
        code: include_str!("code/sample.ts"),
        token: "interface",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "go",
        code: include_str!("code/sample.go"),
        token: "func",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "java",
        code: include_str!("code/sample.java"),
        token: "public",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "python",
        code: include_str!("code/sample.py"),
        token: "def",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "bash",
        code: include_str!("code/sample.sh"),
        token: "if",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "sql",
        code: include_str!("code/sample.sql"),
        token: "SELECT",
        color: Color::Rgb(216, 164, 116),
    },
    Sample {
        language: "yaml",
        code: include_str!("code/sample.yaml"),
        token: "true",
        color: Color::Rgb(194, 162, 195),
    },
    Sample {
        language: "json",
        code: include_str!("code/sample.json"),
        token: "true",
        color: Color::Rgb(194, 162, 195),
    },
    Sample {
        language: "html",
        code: include_str!("code/sample.html"),
        token: "hello",
        color: Color::Rgb(166, 191, 145),
    },
    Sample {
        language: "css",
        code: include_str!("code/sample.css"),
        token: "hello",
        color: Color::Rgb(166, 191, 145),
    },
];
