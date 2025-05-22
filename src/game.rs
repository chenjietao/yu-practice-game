use anyhow::Result;
use crossterm::event::{self, Event};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use tui::{
    backend::CrosstermBackend,
    layout::Alignment,
    widgets::ListState,
    Terminal,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Radical {
    pub code: String,       // 字根编码
    pub text: String,       // 字根本身
    pub frequency: usize,   // 使用频率
    pub big_code: String,   // 大码
    pub small_code: String, // 小码
}

#[derive(Debug, Clone)]
pub struct GameConfig {
    pub radical_file: String,       // 字根文件路径
    pub penalty: usize,             // 错1罚几
    pub practice_mode: PracticeMode, // 练习模式
    pub order: PracticeOrder,       // 练习顺序
    pub mode: GameMode,             // 界面模式(正常/摸鱼)
}



#[derive(Debug, Clone, Copy)]
pub enum PracticeMode {
    BigCode,    // 只练习大码
    DualCode,   // 练习双编码
}

#[derive(Debug, Clone, Copy)]
pub enum PracticeOrder {
    Alphabetical,    // 按字母顺序
    Frequency,      // 按频率顺序
    Keyboard,       // 按键盘顺序
    Random,         // 随机顺序
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameMode {
    Normal,     // 正常模式
    Pretend,    // 摸鱼模式(只改变边框和空白区域)
}

#[derive(Debug)]
pub struct GameState {
    pub radicals: Vec<Radical>,         // 所有字根
    pub current_radical: usize,         // 当前练习的字根索引
    pub remaining_practice: HashMap<String, usize>, // 每个字根剩余练习次数
    pub correct_count: usize,           // 正确计数
    pub wrong_count: usize,             // 错误计数
    pub total_practice: usize,          // 总练习次数
    pub last_error: Option<String>,     // 最后错误信息
    pub recent_radicals: Vec<String>,    // 最近练习的字根(最多3个)
}

impl GameConfig {
    /// 显示设置菜单并获取用户选择
    pub fn show_settings_menu(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<Self> {
        use crossterm::event::KeyCode;
        use tui::{
            layout::{Constraint, Direction, Layout},
            widgets::{Block, Borders, List, ListItem, Paragraph},
            style::{Style, Color},
        };

                let mut selected_item = 0;
                let mut config = Self {
                    radical_file: "res/yujoy-3.8.0.txt".to_string(),
                    penalty: 4,
                    practice_mode: PracticeMode::DualCode,
                    order: PracticeOrder::Random,
                    mode: GameMode::Normal,
                };

                loop {
                    terminal.draw(|f| {
                        let size = f.size();
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .margin(1)
                            .constraints([
                                Constraint::Length(3),
                                Constraint::Min(10),
                                Constraint::Length(3),
                            ])
                            .split(size);

                        // 标题
                        let title = Paragraph::new("设置菜单")
                            .block(Block::default().borders(Borders::ALL))
                            .alignment(tui::layout::Alignment::Center);
                        f.render_widget(title, chunks[0]);

                        // 设置选项
                        let settings_items = vec![
                            ListItem::new(format!("字根文件: {}", config.radical_file)),
                            ListItem::new(format!("错误惩罚: {}次", config.penalty)),
                            ListItem::new(format!("练习模式: {}", match config.practice_mode {
                                PracticeMode::BigCode => "仅大码",
                                PracticeMode::DualCode => "双编码",
                            })),
                            ListItem::new(format!("练习顺序: {}", match config.order {
                                PracticeOrder::Alphabetical => "字母顺序",
                                PracticeOrder::Frequency => "频率顺序",
                                PracticeOrder::Keyboard => "键盘顺序",
                                PracticeOrder::Random => "随机顺序",
                            })),
                            ListItem::new(format!("界面模式: {}", match config.mode {
                                GameMode::Normal => "正常模式",
                                GameMode::Pretend => "摸鱼模式",
                            })),
                        ];

                        let mut state = ListState::default();
                        state.select(Some(selected_item));
                        let settings_list = List::new(settings_items)
                            .block(Block::default().borders(Borders::ALL).title("设置选项"))
                            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
                            .highlight_symbol(">> ");
                        f.render_stateful_widget(settings_list, chunks[1], &mut state);

                        // 操作提示
                        let help = Paragraph::new("↑/↓: 选择选项 | ←/→: 修改选项 | Enter: 确认 | ESC: 取消")
                            .block(Block::default().borders(Borders::ALL))
                            .alignment(tui::layout::Alignment::Center);
                        f.render_widget(help, chunks[2]);
                    })?;

                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Up => {
                                if selected_item > 0 {
                                    selected_item -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if selected_item < 4 {
                                    selected_item += 1;
                                }
                            }
                            KeyCode::Left | KeyCode::Right => {
                                match selected_item {
                                    0 => {
                                        let files = ["res/yujoy-3.8.0.txt", "res/yulight-3.8.0.txt", "res/yustar-3.8.0.txt", "按右方向键手动输入→"];
                                        let current_index = files.iter().position(|&f| f == config.radical_file.as_str())
                                            .unwrap_or(files.len() - 1);
                                        
                                        match key.code {
                                            KeyCode::Left => {
                                                if current_index > 0 {
                                                    config.radical_file = files[current_index - 1].to_string();
                                                } else if current_index == 0 {
                                                    config.radical_file = files[files.len() - 1].to_string();
                                                }
                                                
                                                // 从手动输入切换回文件时清除输入内容
                                                if current_index == files.len() - 1 {
                                                    config.radical_file = files[files.len() - 2].to_string();
                                                }
                                            }
                                            KeyCode::Right => {
                                                if current_index < files.len() - 1 {
                                                    config.radical_file = files[current_index + 1].to_string();
                                                } else if current_index == files.len() - 1 {
                                                    // 进入手动输入模式
                                                    let mut input = String::new();
                                                    terminal.draw(|f| {
                                                        let size = f.size();
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
                                                            match key.code {
                                                                KeyCode::Char(c) => {
                                                                    input.push(c);
                                                                }
                                                                KeyCode::Backspace => {
                                                                    input.pop();
                                                                }
                                                                KeyCode::Enter => {
                                                                    if !input.is_empty() {
                                                                        config.radical_file = input.trim().to_string();
                                                                        break;
                                                                    }
                                                                }
                                                                KeyCode::Esc => {
                                                                    config.radical_file = files[files.len() - 2].to_string();
                                                                    break;
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                        terminal.draw(|f| {
                                                            let size = f.size();
                                                            let block = Block::default()
                                                                .title(format!("输入字根文件路径: {}", input))
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
                                    1 => config.penalty = match (config.penalty, key.code) {
                                        (1, KeyCode::Left) => 1,
                                        (n, KeyCode::Left) => n - 1,
                                        (10, KeyCode::Right) => 10,
                                        (n, KeyCode::Right) => n + 1,
                                        _ => config.penalty,
                                    },
                                    2 => config.practice_mode = match key.code {
                                        KeyCode::Left => match &config.practice_mode {
                                            PracticeMode::BigCode => PracticeMode::BigCode,
                                            PracticeMode::DualCode => PracticeMode::BigCode,
                                        },
                                        KeyCode::Right => match &config.practice_mode {
                                            PracticeMode::BigCode => PracticeMode::DualCode,
                                            PracticeMode::DualCode => PracticeMode::DualCode,
                                        },
                                        _ => config.practice_mode,
                                    },
                                    3 => config.order = match key.code {
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
                                    },
                                    4 => config.mode = match key.code {
                                        KeyCode::Left => match &config.mode {
                                            GameMode::Normal => GameMode::Normal,
                                            GameMode::Pretend => GameMode::Normal,
                                        },
                                        KeyCode::Right => match &config.mode {
                                            GameMode::Normal => GameMode::Pretend,
                                            GameMode::Pretend => GameMode::Pretend,
                                        },
                                        _ => config.mode,
                                    },
                                    _ => {}
                                }
                            }
                            KeyCode::Enter => {
                                return Ok(config);
                            }
                            KeyCode::Esc => {
                                return Ok(Self {
                                    radical_file: "res/yujoy-3.8.0.txt".to_string(),
                                    penalty: 4,
                                    practice_mode: PracticeMode::DualCode,
                                    order: PracticeOrder::Random,
                                    mode: GameMode::Normal,
                                });
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
                
                // 假设双编码格式为"大码+小码"
                let big_code = code.chars().next().unwrap().to_string();
                let small_code = if code.len() > 1 {
                    code.chars().nth(1).unwrap().to_string()
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
                    let a_pos = keyboard_order.find(a_first_char.to_lowercase().next().unwrap_or_default()).unwrap_or(usize::MAX);
                    let b_pos = keyboard_order.find(b_first_char.to_lowercase().next().unwrap_or_default()).unwrap_or(usize::MAX);
                    
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
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                let mut radicals = radicals;
                radicals.shuffle(&mut rng);
                radicals
            }
        };

        // 初始化每个字根的练习次数
        let mut remaining_practice = HashMap::new();
        for radical in &radicals {
            remaining_practice.insert(radical.text.clone(), 2); // 默认每个字根练习2次
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
        let rank = sorted.iter().position(|r| r.text == radical.text).map_or(0, |p| p + 1);
        
        (radical.frequency, percentage, rank)
    }

    /// 生成随机字符用于摸鱼模式的空白区域(无边框)
    pub fn generate_pretend_chars(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let text_chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
        
        // 生成高密度字符填充(覆盖80%界面)
        let mut result = String::new();
        let line_count = rng.gen_range(20..30); // 20-30行
        
        for _ in 0..line_count {
            // 每行生成30-50个随机字符
            let chars_per_line = rng.gen_range(30..50);
            for _ in 0..chars_per_line {
                result.push(text_chars[rng.gen_range(0..text_chars.len())]);
            }
            result.push('\n');
            
            // 添加少量空白行(10%概率)
            if rng.gen_bool(0.1) {
                result.push('\n');
            }
        }
        
        // 确保最后一行也有完整字符
        let last_line_chars = rng.gen_range(30..50);
        for _ in 0..last_line_chars {
            result.push(text_chars[rng.gen_range(0..text_chars.len())]);
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
        let current_radical_text = self.current_radical().map(|r| r.text.clone());

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
                self.remaining_practice.entry(text)
                    .and_modify(|c| *c = c.saturating_sub(1));
            } else {
                self.wrong_count += 1;
                self.remaining_practice.entry(text)
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
                indices.sort_by(|&a, &b| self.radicals[b].frequency.cmp(&self.radicals[a].frequency));
                indices
            }
            PracticeOrder::Keyboard => {
                // 按键盘顺序
                (0..self.radicals.len()).collect()
            }
            PracticeOrder::Random => {
                // 随机顺序
                let mut indices: Vec<usize> = (0..self.radicals.len()).collect();
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                indices.shuffle(&mut rng);
                indices
            }
        };

        // 生成随机间隔(3-6)
        use rand::Rng;
        let random_interval = rand::thread_rng().gen_range(3..=6);

        // 过滤掉不需要练习或最近随机间隔内练习过的字根
        candidates.retain(|&i| {
            if let Some(radical) = self.radicals.get(i) {
                let should_retain = self.remaining_practice.get(&radical.text).map_or(false, |&c| c > 0) &&
                    !self.recent_radicals.iter().take(random_interval).any(|r| r == &radical.text);
                
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
                        self.remaining_practice.get(&radical.text).map_or(false, |&c| c > 0)
                    } else {
                        false
                    }
                })
                .collect();
        }

        // 选择下一个字根
        if let Some(&next_index) = candidates.first() {
            // 更新最近练习的字根列表
            if let Some(radical) = self.radicals.get(next_index) {
                self.recent_radicals.insert(0, radical.text.clone());
                if self.recent_radicals.len() > 6 {
                    self.recent_radicals.pop();
                }
            }
            self.current_radical = next_index;
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
        let total: usize = self.remaining_practice.values().sum::<usize>() + self.correct_count + self.wrong_count;
        let completed = self.correct_count + self.wrong_count;
        (completed, total)
    }
}
