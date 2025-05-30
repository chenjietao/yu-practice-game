use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use rand::{seq::SliceRandom, Rng, rng};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Radical {
    pub code: String,       // 字根编码
    pub text: String,       // 字根本身
    pub frequency: usize,   // 使用频率
    pub big_code: String,   // 大码
    pub small_code: String, // 小码
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub radical_file: String,        // 字根文件路径
    pub frequency_file: String,      // 频率文件路径
    pub penalty: usize,              // 错1罚几
    pub min_practice_count: usize,    // 最小练习次数(1-5)
    pub practice_mode: PracticeMode, // 练习模式
    pub order: PracticeOrder,        // 练习顺序
    pub mode: GameMode,              // 界面模式(正常/摸鱼)
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PracticeMode {
    BigCode,  // 只练习大码
    DualCode, // 练习双编码
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PracticeOrder {
    Alphabetical, // 按字母顺序
    Frequency,    // 按频率顺序
    Keyboard,     // 按键盘顺序
    Random,       // 随机顺序
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GameMode {
    Normal,  // 正常模式
    Pretend, // 摸鱼模式(只改变边框和空白区域)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameState {
    pub radicals: Vec<Radical>,                     // 所有字根
    pub current_radical: usize,                     // 当前练习的字根索引
    pub remaining_practice: HashMap<String, usize>, // 每个字根剩余练习次数
    pub correct_count: usize,                       // 正确计数
    pub wrong_count: usize,                         // 错误计数
    pub total_practice: usize,                      // 总练习次数
    pub last_error: Option<String>,                 // 最后错误信息
    pub recent_radicals: Vec<String>,               // 最近练习的字根(最多6个)
    pub last_big_code: Option<String>,              // 上一个字根的大码(用于键盘高亮)
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveData {
    radicals: Vec<Radical>,
    current_radical: usize,
    remaining_practice: HashMap<String, usize>,
    correct_count: usize,
    wrong_count: usize,
    total_practice: usize,
    recent_radicals: Vec<String>,
    config: GameConfig,
}

impl GameState {
    pub fn save_to_file(&self, config: &GameConfig) -> Result<()> {
        let save_data = SaveData {
            radicals: self.radicals.clone(),
            current_radical: self.current_radical,
            remaining_practice: self.remaining_practice.clone(),
            correct_count: self.correct_count,
            wrong_count: self.wrong_count,
            total_practice: self.total_practice,
            recent_radicals: self.recent_radicals.clone(),
            config: config.clone(),
        };

        let serialized = serde_json::to_string(&save_data)?;
        fs::write("save.json", serialized)?;
        Ok(())
    }

    pub fn load_from_file() -> Option<(Self, GameConfig)> {
        if let Ok(data) = fs::read_to_string("save.json") {
            if let Ok(save_data) = serde_json::from_str::<SaveData>(&data) {
                return Some((
                    GameState {
                        radicals: save_data.radicals,
                        current_radical: save_data.current_radical,
                        remaining_practice: save_data.remaining_practice,
                        correct_count: save_data.correct_count,
                        wrong_count: save_data.wrong_count,
                        total_practice: save_data.total_practice,
                        last_error: None,
                        recent_radicals: save_data.recent_radicals,
                        last_big_code: None,
                    },
                    save_data.config,
                ));
            }
        }
        None
    }
}

impl GameConfig {
    /// 显示设置菜单并获取用户选择
    pub fn show_settings_menu(
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<Self> {
        let mut selected_item = 0;
        let mut config = Self {
            radical_file: "res/yujoy-3.8.0.txt".to_string(),
            frequency_file: "res/counts.txt".to_string(),
            penalty: 4,
            min_practice_count: 2,    // 默认最小练习次数为2
            practice_mode: PracticeMode::DualCode,
            order: PracticeOrder::Random,
            mode: GameMode::Normal,
            cancelled: false,
        };

        loop {
            terminal.draw(|f| {
                let size = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(8),
                        Constraint::Length(3),
                    ])
                    .split(size);

                // 标题
                let title = Paragraph::new("设置菜单")
                    .block(Block::default().borders(Borders::ALL))
                    .alignment(Alignment::Center);
                f.render_widget(title, chunks[0]);

                // 设置选项
                let settings_items = vec![
                    ListItem::new(format!("字根文件: {}", config.radical_file)),
                    ListItem::new(format!("频率文件: {}", config.frequency_file)),
                    ListItem::new(format!("错误惩罚: {}次", config.penalty)),
                    ListItem::new(format!("最少练习: {}次", config.min_practice_count)),
                    ListItem::new(format!(
                        "练习模式: {}",
                        match config.practice_mode {
                            PracticeMode::BigCode => "仅大码",
                            PracticeMode::DualCode => "大小码",
                        }
                    )),
                    ListItem::new(format!(
                        "练习顺序: {}",
                        match config.order {
                            PracticeOrder::Alphabetical => "字母顺序",
                            PracticeOrder::Frequency => "频率顺序",
                            PracticeOrder::Keyboard => "键盘顺序",
                            PracticeOrder::Random => "随机顺序",
                        }
                    )),
                    ListItem::new(format!(
                        "界面模式: {}",
                        match config.mode {
                            GameMode::Normal => "正常模式",
                            GameMode::Pretend => "摸鱼模式(界面空白区域使用随机字符填充)",
                        }
                    )),
                ];

                let mut state = ListState::default();
                state.select(Some(selected_item));
                let settings_list = List::new(settings_items)
                    .block(Block::default().borders(Borders::ALL).title("设置选项"))
                    .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
                    .highlight_symbol(">> ");
                f.render_stateful_widget(settings_list, chunks[1], &mut state);

                // 操作提示
                let help =
                    Paragraph::new("↑/↓: 选择选项 | ←/→: 修改选项 | Enter: 确认 | ESC: 取消")
                        .block(Block::default().borders(Borders::ALL))
                        .alignment(Alignment::Center);
                f.render_widget(help, chunks[2]);
            })?;

            if let Event::Key(key) = event::read()? {
                #[cfg(windows)]
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Up => {
                        if selected_item > 0 {
                            selected_item -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if selected_item < 6 {
                            selected_item += 1;
                        }
                    }
                    KeyCode::Left | KeyCode::Right => {
                        match selected_item {
                            0 => {
                                let files = [
                                    "res/yujoy-3.8.0.txt",
                                    "res/yulight-3.8.0.txt",
                                    "res/yustar-3.8.0.txt",
                                    "res/yujoy-3.6.0.txt",
                                    "res/yusm-3.9.0-20250522.txt",
                                    "按右方向键手动输入→",
                                ];
                                let current_idx = files
                                    .iter()
                                    .position(|&f| f == config.radical_file.as_str())
                                    .unwrap_or(files.len() - 1);

                                match key.code {
                                    KeyCode::Left => {
                                        if current_idx > 0 {
                                            config.radical_file =
                                                files[current_idx - 1].to_string();
                                        } else if current_idx == 0 {
                                            config.radical_file =
                                                files[files.len() - 1].to_string();
                                        }

                                        // 从手动输入切换回文件时清除输入内容
                                        if current_idx == files.len() - 1 {
                                            config.radical_file =
                                                files[files.len() - 2].to_string();
                                        }
                                    }
                                    KeyCode::Right => {
                                        if current_idx < files.len() - 1 {
                                            config.radical_file =
                                                files[current_idx + 1].to_string();
                                        } else if current_idx == files.len() - 1 {
                                            // 进入手动输入模式
                                            let mut input = String::new();
                                            terminal.draw(|f| {
                                                let size = f.area();
                                                let block = Block::default()
                                                    .title("输入字根文件路径 (Enter确认, ESC取消)")
                                                    .borders(Borders::ALL);
                                                let input_text = Paragraph::new(input.as_str())
                                                    .block(block)
                                                    .alignment(Alignment::Center);
                                                f.render_widget(input_text, size);
                                            })?;

                                            loop {
                                                if let Event::Key(key) = event::read()? {
                                                    #[cfg(windows)]
                                                    if key.kind
                                                        != event::KeyEventKind::Press
                                                    {
                                                        continue;
                                                    }
                                                    match key.code {
                                                        KeyCode::Char(c) => {
                                                            input.push(c);
                                                        }
                                                        KeyCode::Backspace => {
                                                            input.pop();
                                                        }
                                                        KeyCode::Enter => {
                                                            if !input.is_empty() {
                                                                config.radical_file =
                                                                    input.trim().to_string();
                                                                break;
                                                            }
                                                        }
                                                        KeyCode::Esc => {
                                                            config.radical_file =
                                                                files[files.len() - 2].to_string();
                                                            break;
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                                terminal.draw(|f| {
                                                    let size = f.area();
                                                    let block = Block::default()
                                                        .title("输入字根文件路径 (Enter确认, ESC取消)")
                                                        .borders(Borders::ALL);
                                                    let input_text = Paragraph::new(input.as_str())
                                                        .block(block)
                                                        .alignment(Alignment::Center);
                                                    f.render_widget(input_text, size);
                                                })?;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            1 => {
                                // 频率文件逻辑（与字根文件类似）
                                let files = [
                                    "res/counts.txt",
                                    "res/counts-3.6.0.txt",
                                    "按右方向键手动输入→",
                                ];
                                let current_idx = files
                                    .iter()
                                    .position(|&f| f == config.frequency_file.as_str())
                                    .unwrap_or(files.len() - 1);

                                match key.code {
                                    KeyCode::Left => {
                                        if current_idx > 0 {
                                            config.frequency_file =
                                                files[current_idx - 1].to_string();
                                        } else if current_idx == 0 {
                                            config.frequency_file =
                                                files[files.len() - 1].to_string();
                                        }
                                        if current_idx == files.len() - 1 {
                                            config.frequency_file =
                                                files[files.len() - 2].to_string();
                                        }
                                    }
                                    KeyCode::Right => {
                                        if current_idx < files.len() - 1 {
                                            config.frequency_file =
                                                files[current_idx + 1].to_string();
                                        } else if current_idx == files.len() - 1 {
                                            // 手动输入
                                            let mut input = String::new();
                                            terminal.draw(|f| {
                                                let size = f.area();
                                                let block = Block::default()
                                                    .title("输入频率文件路径 (Enter确认, ESC取消)")
                                                    .borders(Borders::ALL);
                                                let input_text = Paragraph::new(input.as_str())
                                                    .block(block)
                                                    .alignment(Alignment::Center);
                                                f.render_widget(input_text, size);
                                            })?;
                                            loop {
                                                if let Event::Key(key) = event::read()? {
                                                    #[cfg(windows)]
                                                    if key.kind
                                                        != event::KeyEventKind::Press
                                                    {
                                                        continue;
                                                    }
                                                    match key.code {
                                                        KeyCode::Char(c) => input.push(c),
                                                        KeyCode::Backspace => {
                                                            input.pop();
                                                        }
                                                        KeyCode::Enter => {
                                                            if !input.is_empty() {
                                                                config.frequency_file =
                                                                    input.trim().to_string();
                                                                break;
                                                            }
                                                        }
                                                        KeyCode::Esc => {
                                                            config.frequency_file =
                                                                files[files.len() - 2].to_string();
                                                            break;
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                                terminal.draw(|f| {
                                                    let size = f.area();
                                                    let block = Block::default()
                                                        .title("输入频率文件路径 (Enter确认, ESC取消)")
                                                        .borders(Borders::ALL);
                                                    let input_text = Paragraph::new(input.as_str())
                                                        .block(block)
                                                        .alignment(Alignment::Center);
                                                    f.render_widget(input_text, size);
                                                })?;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            2 => {
                                config.penalty = match (config.penalty, key.code) {
                                    (1, KeyCode::Left) => 1,
                                    (n, KeyCode::Left) => n - 1,
                                    (10, KeyCode::Right) => 10,
                                    (n, KeyCode::Right) => n + 1,
                                    _ => config.penalty,
                                }
                            }
                            3 => {
                                config.min_practice_count = match (config.min_practice_count, key.code) {
                                    (1, KeyCode::Left) => 1,
                                    (n, KeyCode::Left) => n - 1,
                                    (5, KeyCode::Right) => 5,
                                    (n, KeyCode::Right) => n + 1,
                                    _ => config.min_practice_count,
                                }
                            }
                            4 => {
                                config.practice_mode = match key.code {
                                    KeyCode::Left => match &config.practice_mode {
                                        PracticeMode::BigCode => PracticeMode::BigCode,
                                        PracticeMode::DualCode => PracticeMode::BigCode,
                                    },
                                    KeyCode::Right => match &config.practice_mode {
                                        PracticeMode::BigCode => PracticeMode::DualCode,
                                        PracticeMode::DualCode => PracticeMode::DualCode,
                                    },
                                    _ => config.practice_mode,
                                }
                            }
                            5 => {
                                config.order = match key.code {
                                    KeyCode::Left => match &config.order {
                                        PracticeOrder::Alphabetical => PracticeOrder::Alphabetical,
                                        PracticeOrder::Frequency => PracticeOrder::Alphabetical,
                                        PracticeOrder::Keyboard => PracticeOrder::Frequency,
                                        PracticeOrder::Random => PracticeOrder::Keyboard,
                                    },
                                    KeyCode::Right => match &config.order {
                                        PracticeOrder::Alphabetical => PracticeOrder::Frequency,
                                        PracticeOrder::Frequency => PracticeOrder::Keyboard,
                                        PracticeOrder::Keyboard => PracticeOrder::Random,
                                        PracticeOrder::Random => PracticeOrder::Random,
                                    },
                                    _ => config.order,
                                }
                            }
                            6 => {
                                config.mode = match key.code {
                                    KeyCode::Left => match &config.mode {
                                        GameMode::Normal => GameMode::Normal,
                                        GameMode::Pretend => GameMode::Normal,
                                    },
                                    KeyCode::Right => match &config.mode {
                                        GameMode::Normal => GameMode::Pretend,
                                        GameMode::Pretend => GameMode::Pretend,
                                    },
                                    _ => config.mode,
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Enter => {
                        return Ok(config);
                    }
                    KeyCode::Esc => {
                        config.cancelled = true;
                        return Ok(config)
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Radical {
    /// 从文件加载字根数据
    pub fn load_from_files(counts_file: &str, code_file: &str) -> Result<Vec<Self>> {
        // 加载字根频率数据
        let frequency_map = Self::load_frequency_data(counts_file)?;

        // 加载字根编码数据
        let mut radicals = Self::load_code_data(code_file)?;

        // 合并频率数据
        for radical in &mut radicals {
            if let Some(&freq) = frequency_map.get(&radical.text) {
                radical.frequency = freq;
            }
        }

        Ok(radicals)
    }

    fn load_frequency_data(path: &str) -> Result<HashMap<String, usize>> {
        let content = fs::read_to_string(path)?;
        let mut map = HashMap::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let text = parts[0].to_string();
                let freq = parts[1].parse().unwrap_or(0);
                map.insert(text, freq);
            }
        }

        Ok(map)
    }

    fn load_code_data(path: &str) -> Result<Vec<Self>> {
        let content = fs::read_to_string(path)?;
        let mut radicals = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let code = parts[0].to_string();
                let text = parts[1].to_string();

                // 大码是第一个字符，小码是剩余字符
                let big_code = code.chars().next().unwrap().to_string();
                let small_code = if code.len() > 1 {
                    code.chars().skip(1).collect()
                } else {
                    "".to_string()
                };

                radicals.push(Self {
                    code,
                    text,
                    frequency: 0, // 初始化为0，后面会合并频率数据
                    big_code,
                    small_code,
                });
            }
        }

        Ok(radicals)
    }
}

impl GameState {
    /// 初始化游戏状态
    pub fn new(mut radicals: Vec<Radical>, config: &GameConfig) -> Self {
        // 根据练习顺序排序字根
        let radicals = match config.order {
            PracticeOrder::Alphabetical => {
                radicals.sort_by(|a, b| a.code.cmp(&b.code));
                radicals
            }
            PracticeOrder::Frequency => {
                radicals.sort_by(|a, b| b.frequency.cmp(&a.frequency));
                radicals
            }
            PracticeOrder::Keyboard => {
                // 按照键盘顺序排序（只比较首字母，次字母按字母顺序）
                let keyboard_order = "asdfghjklqwertyuiopzxcvbnm";
                radicals.sort_by(|a, b| {
                    let a_first_char = a.code.chars().next().unwrap_or_default();
                    let b_first_char = b.code.chars().next().unwrap_or_default();

                    // 比较首字母的键盘位置
                    let a_pos = keyboard_order
                        .find(a_first_char.to_lowercase().next().unwrap_or_default())
                        .unwrap_or(usize::MAX);
                    let b_pos = keyboard_order
                        .find(b_first_char.to_lowercase().next().unwrap_or_default())
                        .unwrap_or(usize::MAX);

                    if a_pos != b_pos {
                        a_pos.cmp(&b_pos)
                    } else {
                        // 首字母相同则按整个编码字母顺序排序
                        a.code.cmp(&b.code)
                    }
                });
                radicals
            }
            PracticeOrder::Random => {
                let mut rng = rng();
                let mut radicals = radicals;
                radicals.shuffle(&mut rng);
                radicals
            }
        };

        // 初始化每个字根的练习次数
        let mut remaining_practice = HashMap::new();
        for radical in &radicals {
            remaining_practice.insert(radical.text.clone(), config.min_practice_count);
        }

        GameState {
            radicals,
            current_radical: 0,
            remaining_practice,
            correct_count: 0,
            wrong_count: 0,
            total_practice: 0,
            last_error: None,
            recent_radicals: Vec::with_capacity(6), // 预分配容量为6以适应随机间隔
            last_big_code: None,
        }
    }

    /// 获取当前练习的字根
    pub fn current_radical(&self) -> Option<&Radical> {
        self.radicals.get(self.current_radical)
    }

    /// 获取频率统计数据
    fn get_frequency_stats(&self, radical: &Radical) -> (usize, f64, usize) {
        // 计算总使用次数
        let total: usize = self.radicals.iter().map(|r| r.frequency).sum();
        // 计算百分比 (千分比)
        let percentage = if total > 0 {
            (radical.frequency as f64 / total as f64) * 1000.0
        } else {
            0.0
        };
        // 计算排名
        let mut sorted = self.radicals.clone();
        sorted.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        let rank = sorted
            .iter()
            .position(|r| r.text == radical.text)
            .map_or(0, |p| p + 1);

        (radical.frequency, percentage, rank)
    }

    /// 生成随机字符用于摸鱼模式的空白区域(无边框)
    pub fn generate_pretend_chars(&self) -> String {
        let mut rng = rng();
        let text_chars: Vec<char> =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                .chars()
                .collect();

        // 生成高密度字符填充(覆盖80%界面)
        let mut result = String::new();
        let line_count = rng.random_range(20..30); // 20-30行

        for _ in 0..line_count {
            // 每行生成30-50个随机字符
            let chars_per_line = rng.random_range(30..50);
            for _ in 0..chars_per_line {
                result.push(text_chars[rng.random_range(0..text_chars.len())]);
            }
            result.push('\n');

            // 添加少量空白行(10%概率)
            if rng.random_bool(0.1) {
                result.push('\n');
            }
        }

        // 确保最后一行也有完整字符
        let last_line_chars = rng.random_range(30..50);
        for _ in 0..last_line_chars {
            result.push(text_chars[rng.random_range(0..text_chars.len())]);
        }

        result
    }

    /// 检查用户输入逻辑
    fn check_input_core(&self, input: &str, config: &GameConfig) -> (bool, Option<String>) {
        // 防御性编程：检查所有前置条件
        if self.radicals.is_empty() {
            return (false, Some("没有可练习的字根".to_string()));
        }

        // 确保current_radical在有效范围内
        let current_idx = self.current_radical.min(self.radicals.len() - 1);
        let radical = &self.radicals[current_idx];

        // 验证输入有效性
        let input = input.trim();
        if input.is_empty() {
            return (false, Some("输入不能为空".to_string()));
        }

        // 安全比较输入（不区分大小写）
        let input_lower = input.to_lowercase();
        let is_correct = match config.practice_mode {
            PracticeMode::BigCode => input_lower == radical.big_code.to_lowercase(),
            PracticeMode::DualCode => input_lower == radical.code.to_lowercase(),
        };

        // 获取频率数据
        let (count, percentage, rank) = self.get_frequency_stats(radical);

        // 生成纯文本提示信息
        let status = if is_correct { "正确" } else { "错误" };
        let message = format!(
            "【{}】“{}”的编码是:{}{}，使用频率为:{}({:.4}‰)，排在第{}位",
            status,
            radical.text,
            radical.big_code.to_uppercase(),
            radical.small_code.to_lowercase(),
            count,
            percentage,
            rank
        );

        (is_correct, Some(message))
    }

    /// 检查用户输入是否正确
    pub fn check_input(&mut self, input: &str, config: &GameConfig) -> bool {
        // 空输入直接返回false且不更新任何状态
        let input = input.trim();
        if input.is_empty() {
            self.last_error = Some("输入不能为空".to_string());
            return false;
        }

        let (is_correct, message) = self.check_input_core(input, config);
        self.last_error = message;

        // 获取当前字根文本
        let current_radical = self.current_radical();
        let current_radical_text = current_radical.map(|r| r.text.clone());

        // 更新上一个字根的大码
        if let Some(radical) = current_radical {
            self.last_big_code = Some(radical.big_code.clone());
        }

        // 更新最近练习的字根列表
        if let Some(text) = &current_radical_text {
            self.recent_radicals.insert(0, text.clone());
            if self.recent_radicals.len() > 6 {
                self.recent_radicals.pop();
            }
        }

        // 更新状态（摸鱼模式和正常模式都更新）
        if let Some(text) = current_radical_text {
            if is_correct {
                self.correct_count += 1;
                self.remaining_practice
                    .entry(text)
                    .and_modify(|c| *c = c.saturating_sub(1));
            } else {
                self.wrong_count += 1;
                self.remaining_practice
                    .entry(text)
                    .and_modify(|c| *c += config.penalty);
            }
            self.total_practice += 1;
        }

        is_correct
    }

    /// 移动到下一个字根
    pub fn next_radical(&mut self, config: &GameConfig) -> bool {
        // 防御性检查：确保有字根可练习
        if self.radicals.is_empty() {
            return false;
        }

        // 根据配置顺序获取候选字根
        let mut candidates: Vec<usize> = match config.order {
            PracticeOrder::Alphabetical => {
                // 按字母顺序
                (0..self.radicals.len()).collect()
            }
            PracticeOrder::Frequency => {
                // 按频率顺序
                let mut indices: Vec<usize> = (0..self.radicals.len()).collect();
                indices
                    .sort_by(|&a, &b| self.radicals[b].frequency.cmp(&self.radicals[a].frequency));
                indices
            }
            PracticeOrder::Keyboard => {
                // 按键盘顺序
                (0..self.radicals.len()).collect()
            }
            PracticeOrder::Random => {
                // 随机顺序
                let mut indices: Vec<usize> = (0..self.radicals.len()).collect();
                let mut rng = rng();
                indices.shuffle(&mut rng);
                indices
            }
        };

        // 生成随机间隔(3-6)
        let random_interval = rng().random_range(3..=6);

        // 过滤掉不需要练习或最近随机间隔内练习过的字根
        candidates.retain(|&i| {
            if let Some(radical) = self.radicals.get(i) {
                let should_retain = self
                    .remaining_practice
                    .get(&radical.text)
                    .map_or(false, |&c| c > 0)
                    && !self
                        .recent_radicals
                        .iter()
                        .take(random_interval)
                        .any(|r| r == &radical.text);

                should_retain
            } else {
                false
            }
        });

        // 如果没有符合条件的字根，则放宽条件
        if candidates.is_empty() {
            candidates = (0..self.radicals.len())
                .filter(|&i| {
                    if let Some(radical) = self.radicals.get(i) {
                        self.remaining_practice
                            .get(&radical.text)
                            .map_or(false, |&c| c > 0)
                    } else {
                        false
                    }
                })
                .collect();
        }

        // 选择下一个字根
        if let Some(&next_idx) = candidates.first() {
            // 更新最近练习的字根列表
            if let Some(radical) = self.radicals.get(next_idx) {
                self.recent_radicals.insert(0, radical.text.clone());
                if self.recent_radicals.len() > 6 {
                    self.recent_radicals.pop();
                }
            }
            self.current_radical = next_idx;
            true
        } else {
            false
        }
    }

    /// 检查游戏是否结束
    pub fn is_game_over(&self) -> bool {
        self.remaining_practice.values().all(|&c| c == 0)
    }

    /// 获取游戏进度
    pub fn progress(&self) -> (usize, usize) {
        // 计算实际总练习次数（初始2次 + 惩罚次数）
        let total: usize =
            self.remaining_practice.values().sum::<usize>() + self.correct_count + self.wrong_count;
        let completed = self.correct_count + self.wrong_count;
        (completed, total)
    }
}
