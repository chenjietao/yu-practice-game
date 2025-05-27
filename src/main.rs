use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use game::{GameConfig, GameMode, GameState, Radical};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::char;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

mod game;

fn main() -> Result<()> {
    // 初始化终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 显示欢迎界面
    show_welcome(&mut terminal)?;

    // 显示设置菜单
    let config = GameConfig::show_settings_menu(&mut terminal)?;

    if config.cancelled {
        // 清理终端
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        return Ok(());
    }

    // 尝试从多个可能的位置加载资源文件
    let mut counts_path = None;
    let mut radical_path = None;

    // 1. 首先尝试从可执行文件目录查找
    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            let exe_counts = parent.join(&config.frequency_file);
            let exe_radical = parent.join(&config.radical_file);

            if exe_counts.exists() && exe_radical.exists() {
                counts_path = Some(exe_counts);
                radical_path = Some(exe_radical);
            }
        }
    }

    // 2. 如果可执行文件目录找不到，尝试从项目根目录查找
    if counts_path.is_none() || radical_path.is_none() {
        let project_counts = Path::new(&config.frequency_file);
        let project_radical = Path::new(&config.radical_file);

        if project_counts.exists() && project_radical.exists() {
            counts_path = Some(project_counts.to_path_buf());
            radical_path = Some(project_radical.to_path_buf());
        }
    }

    // 检查是否找到有效的资源路径
    let (counts_path, radical_path) = match (counts_path, radical_path) {
        (Some(c), Some(r)) => (c, r),
        _ => {
            return Err(anyhow::anyhow!(
                "无法找到资源文件，请确保res目录位于可执行文件目录或项目根目录下"
            ))
        }
    };

    // 加载字根数据
    let radicals = Radical::load_from_files(
        counts_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("无效路径"))?,
        radical_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("无效路径"))?,
    )?;

    // 创建游戏状态
    let mut game_state = GameState::new(radicals, &config);

    // 主游戏循环
    let res = run_app(&mut terminal, config, &mut game_state);

    // 清理终端
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    res
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: GameConfig,
    game_state: &mut GameState,
) -> Result<()> {
    let mut input_buffer = String::new();

    loop {
        terminal.draw(|f| {
            let size = f.area();

            // 在摸鱼模式下，先绘制随机字符背景
            if config.mode == GameMode::Pretend {
                let pretend_text =
                    Paragraph::new(game_state.generate_pretend_chars()).block(Block::default());
                f.render_widget(pretend_text, size);
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // 当前字根
                    Constraint::Length(3), // 输入框
                    Constraint::Length(3), // 错误提示
                    Constraint::Min(3),    // 键盘布局
                    Constraint::Length(3), // 统计信息
                ])
                .split(size);

            // 显示当前字根
            let border_style = match config.mode {
                GameMode::Normal => Borders::ALL,
                GameMode::Pretend => Borders::NONE,
            };

            if let Some(radical) = game_state.current_radical() {
                let radical_block = Block::default().title("当前字根").borders(border_style);
                let radical_text = Paragraph::new(radical.text.clone())
                    .block(radical_block)
                    .alignment(Alignment::Center);
                f.render_widget(radical_text, chunks[0]);
            }

            // 显示输入区域（增加高度）
            let input_block = Block::default()
                .title("输入编码 (Enter确认)")
                .borders(border_style);
            let input_text = Paragraph::new(input_buffer.clone())
                .block(input_block)
                .alignment(Alignment::Center);
            f.render_widget(input_text, chunks[1]);

            // 显示错误提示（带边框）
            let error_block = Block::default().title("提示").borders(border_style);
            let error_text = if let Some(error_msg) = &game_state.last_error {
                let style = if error_msg.starts_with("【正确】") {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };
                Paragraph::new(error_msg.clone())
                    .style(style)
                    .alignment(Alignment::Center)
            } else {
                Paragraph::new("")
            };
            f.render_widget(error_text.block(error_block), chunks[2]);

            // 键盘布局显示
            if config.mode == GameMode::Normal {
                let keyboard_block = Block::default().borders(Borders::NONE);

                // 创建键盘布局行
                let mut rows: Vec<Line> = vec![];

                // 第一行 QWERTYUIOP
                let mut row1 = vec![Span::raw(" ")];
                for c in ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"] {
                    let style = if let Some(big_code) = &game_state.last_big_code {
                        if c == big_code.to_uppercase() {
                            Style::default().fg(Color::White).bg(Color::Cyan)
                        } else {
                            Style::default()
                        }
                    } else {
                        Style::default()
                    };
                    row1.push(Span::styled(format!("[{}]", c), style));
                    row1.push(Span::raw(" "));
                }
                rows.push(Line::from(row1));

                // 第二行 ASDFGHJKL
                let mut row2 = vec![Span::raw(" ")];
                for c in ["A", "S", "D", "F", "G", "H", "J", "K", "L"] {
                    let style = if let Some(big_code) = &game_state.last_big_code {
                        if c == big_code.to_uppercase() {
                            Style::default().fg(Color::White).bg(Color::Cyan)
                        } else {
                            Style::default()
                        }
                    } else {
                        Style::default()
                    };
                    row2.push(Span::styled(format!("[{}]", c), style));
                    row2.push(Span::raw(" "));
                }
                row2.push(Span::raw("  "));
                rows.push(Line::from(row2));

                // 第三行 ZXCVBNM
                let mut row3 = vec![Span::raw(" ")];
                for c in ["Z", "X", "C", "V", "B", "N", "M"] {
                    let style = if let Some(big_code) = &game_state.last_big_code {
                        if c == big_code.to_uppercase() {
                            Style::default().fg(Color::White).bg(Color::Cyan)
                        } else {
                            Style::default()
                        }
                    } else {
                        Style::default()
                    };
                    row3.push(Span::styled(format!("[{}]", c), style));
                    row3.push(Span::raw(" "));
                }
                row3.push(Span::raw("       "));
                rows.push(Line::from(row3));

                let keyboard = Paragraph::new(rows)
                    .block(keyboard_block)
                    .alignment(Alignment::Center);
                f.render_widget(keyboard, chunks[3]);
            }

            #[cfg(not(target_os = "macos"))]
            let quit_key = "ESC/Alt+Q";
            #[cfg(target_os = "macos")]
            let quit_key = "ESC/Control+Q";
            // 显示进度和统计
            let stats = format!(
                "进度: {}/{} | 正确: {} | 错误: {} | 退出: {}",
                game_state.progress().0,
                game_state.progress().1,
                game_state.correct_count,
                game_state.wrong_count,
                quit_key
            );
            let stats_block = Block::default().title("统计信息").borders(border_style);
            let stats_text = Paragraph::new(stats).block(stats_block);
            f.render_widget(stats_text, chunks[4]);
        })?;

        // 处理用户输入
        if let Event::Key(key) = event::read()? {
            #[cfg(windows)]
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            match key.code {
                #[cfg(not(target_os = "macos"))]
                KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::ALT) => {
                    return Ok(());
                }
                #[cfg(target_os = "macos")]
                KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    input_buffer.pop();
                }
                KeyCode::Enter => {
                    if !input_buffer.is_empty() {
                        let is_correct = game_state.check_input(&input_buffer, &config);
                        input_buffer.clear();

                        // 根据结果给出反馈
                        if is_correct {
                            // 正确，检查是否需要切换到下一个字根
                            if !game_state.next_radical(&config) && game_state.is_game_over() {
                                // 游戏结束
                                let _ = show_message(terminal, "恭喜完成所有练习!");
                                return Ok(());
                            }
                        } else if let Some(_radical) = game_state.current_radical() {
                        } else {
                            game_state.last_error = None;
                        }
                    }
                }
                KeyCode::Esc => {
                    return Ok(());
                }
                _ => {}
            }
        }

        // 只在需要控制游戏节奏的地方添加延迟
    }
}

fn show_welcome(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    terminal.draw(|f| {
        let size = f.area();
        let block = Block::default()
            .title("宇浩字根练习")
            .borders(Borders::ALL);
        let welcome_text = Paragraph::new(vec![
            Line::from("欢迎使用宇浩字根练习工具"),
            Line::from(""),
            Line::from(Span::styled(
                format!("版本: {}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(Color::Gray),
            )),
            Line::from(""),
            Line::from("按任意键继续..."),
            Line::from("按 Z 键进入字根编码转换..."),
        ])
        .block(block)
        .alignment(Alignment::Center);
        f.render_widget(welcome_text, size);
    })?;

    loop {
        if let Event::Key(key) = event::read()? {
            #[cfg(windows)]
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            if key.code == KeyCode::Char('z') || key.code == KeyCode::Char('Z') {
                return show_conversion_ui(terminal);
            }
            break;
        }
    }
    Ok(())
}

fn show_conversion_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut input_fields = vec![
        (String::from("./yustar_chaifen.dict.yaml"), 0), // (文本内容, 光标位置)
        (String::from("res/yucode-custom.txt"), 0),
        (String::from("res/counts-custom.txt"), 0),
    ];
    // 初始化光标位置到末尾
    for (text, pos) in &mut input_fields {
        *pos = text.len();
    }
    enum FocusState {
        InputField(usize),
        Button(bool), // true for confirm, false for cancel
    }
    let mut focus_state = FocusState::InputField(0);

    loop {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(4), // 标题
                    Constraint::Length(5), // 文件路径
                    Constraint::Length(3), // 按钮
                    Constraint::Min(1),    // 空白区域
                ])
                .split(size);

            // 标题
            let title = Paragraph::new(
                "字根编码转换，支持从宇浩单字拆分表导出字根编码和字根使用频率",
            )
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);
            f.render_widget(title, chunks[0]);

            // 输入框列表
            let items: Vec<_> = input_fields
                .iter()
                .enumerate()
                .map(|(i, (text, pos))| {
                    let mut spans = Vec::new();
                    let label = match i {
                        0 => "拆分表文件: ",
                        1 => "编码文件: ",
                        2 => "频率文件: ",
                        _ => "",
                    };
                    spans.push(Span::raw(label));
                    for (idx, ch) in text.chars().enumerate() {
                        if *pos == idx && matches!(focus_state, FocusState::InputField(j) if j == i) {
                            // 高亮当前光标字符
                            spans.push(Span::styled(
                                ch.to_string(),
                                Style::default().fg(Color::White).bg(Color::Black),
                            ));
                        } else {
                            spans.push(Span::raw(ch.to_string()));
                        }
                    }
                    // 如果光标在末尾，显示一个高亮空格
                    if *pos == text.len() && matches!(focus_state, FocusState::InputField(j) if j == i) {
                        spans.push(Span::styled(
                            " ".to_string(),
                            Style::default().fg(Color::White).bg(Color::Black),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();

            let selected = match focus_state {
                FocusState::InputField(idx) => Some(idx),
                _ => None,
            };
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL))
                // .highlight_style(Style::default().bg(Color::Blue))
                .highlight_symbol(">> ");
            f.render_stateful_widget(
                list,
                chunks[1],
                &mut ListState::default().with_selected(selected),
            );

            // 按钮
            let button_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[2]);

            let confirm_button = Paragraph::new("[确认]")
                .block(Block::default().borders(Borders::ALL))
                .style(match focus_state {
                    FocusState::Button(true) => Style::default().fg(Color::White).bg(Color::Green),
                    _ => Style::default(),
                })
                .alignment(Alignment::Center);
            f.render_widget(confirm_button, button_chunks[0]);

            let cancel_button = Paragraph::new("[取消]")
                .block(Block::default().borders(Borders::ALL))
                .style(match focus_state {
                    FocusState::Button(false) => Style::default().fg(Color::White).bg(Color::Red),
                    _ => Style::default(),
                })
                .alignment(Alignment::Center);
            f.render_widget(cancel_button, button_chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            #[cfg(windows)]
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up => {
                    match focus_state {
                        FocusState::InputField(idx) => {
                            // 先将当前输入框光标移动到末尾
                            let (text, pos) = &mut input_fields[idx];
                            *pos = text.len();
                            // 再切换到上一个输入框
                            if idx > 0 {
                                focus_state = FocusState::InputField(idx - 1);
                            }
                        }
                        FocusState::Button(_) => {
                            // 从按钮切换到输入框时，先将最后一个输入框光标移动到末尾
                            let (text, pos) = &mut input_fields[2];
                            *pos = text.len();
                            focus_state = FocusState::InputField(2);
                        }
                    }
                }
                KeyCode::Down => {
                    match focus_state {
                        FocusState::InputField(idx) => {
                            // 先将当前输入框光标移动到末尾
                            let (text, pos) = &mut input_fields[idx];
                            *pos = text.len();
                            // 再切换到下一个输入框或按钮
                            if idx < 2 {
                                focus_state = FocusState::InputField(idx + 1);
                            } else {
                                focus_state = FocusState::Button(true);
                            }
                        }
                        FocusState::Button(_) => {
                            focus_state = FocusState::Button(false);
                        }
                    }
                }
                KeyCode::Left => {
                    match focus_state {
                        FocusState::InputField(idx) => {
                            // 在输入框内左移光标
                            let (_text, pos) = &mut input_fields[idx];
                            if *pos > 0 {
                                *pos -= 1;
                            }
                        }
                        FocusState::Button(_) => {
                            // 左右键在按钮间切换
                            focus_state = FocusState::Button(true);
                        }
                    }
                }
                KeyCode::Right => {
                    match focus_state {
                        FocusState::InputField(idx) => {
                            // 在输入框内右移光标
                            let (text, pos) = &mut input_fields[idx];
                            if *pos < text.len() {
                                *pos += 1;
                            }
                        }
                        FocusState::Button(_) => {
                            // 左右键在按钮间切换
                            focus_state = FocusState::Button(false);
                        }
                    }
                }
                KeyCode::Enter => {
                    match focus_state {
                        FocusState::Button(true) => {
                            // 确认按钮被选中 - 执行转换后直接退出
                            convert_radicals(
                                &input_fields[0].0,
                                &input_fields[1].0,
                                &input_fields[2].0,
                            )?;
                            return show_welcome(terminal);
                        }
                        FocusState::Button(false) => {
                            // 取消按钮被选中 - 返回欢迎界面
                            return show_welcome(terminal);
                        }
                        _ => {}
                    }
                }
                KeyCode::Char(c) => {
                    if let FocusState::InputField(idx) = focus_state {
                        let (text, pos) = &mut input_fields[idx];
                        text.insert(*pos, c);
                        *pos += 1;
                    }
                }
                KeyCode::Backspace => {
                    if let FocusState::InputField(idx) = focus_state {
                        let (text, pos) = &mut input_fields[idx];
                        if *pos > 0 {
                            text.remove(*pos - 1);
                            *pos -= 1;
                        }
                    }
                }
                KeyCode::Esc => {
                    return show_welcome(terminal);
                }
                _ => {}
            }
        }
    }
}

fn convert_radicals(
    input_path: &str,
    code_output_path: &str,
    counts_output_path: &str,
) -> Result<()> {
    // 检查输入文件是否存在
    if !Path::new(input_path).exists() {
        return Err(anyhow::anyhow!("拆分表文件不存在: {}", input_path));
    }

    // 读取输入文件
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);

    let mut radical_counts: HashMap<String, u32> = HashMap::new();
    let mut radical_codes: HashMap<String, String> = HashMap::new();
    let mut processing = false;
    let mut is_sun_moon = false;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        // 跳过注释行和空行
        if line.is_empty() || line.starts_with('#') {
            if !is_sun_moon && (line.starts_with("# 日月") || line.starts_with("# 宇浩日月"))
            {
                is_sun_moon = true
            }
            continue;
        }

        // 检查是否到达"..."行
        if !processing && line.starts_with("...") {
            processing = true;
            continue;
        }

        if processing {
            // 解析行格式：汉字\t[拆分,编码,拼音,字符集,unicode]
            let tab_start = line.find('\t').unwrap_or(2);
            if let Some(bracket_start) = line[tab_start..].find('[').map(|i| i + tab_start) {
                if let Some(bracket_end) = line[tab_start..].find(']').map(|i| i + tab_start) {
                    let content = &line[bracket_start + 1..bracket_end];
                    let counting = content.contains("CJK");
                    let parts: Vec<&str> = content.split(',').collect();
                    if parts.len() >= 2 {
                        let radicals = parts[0].trim(); // 拆分部分
                        let codes = parts[1].trim(); // 编码部分

                        if radicals.is_empty() {
                            continue;
                        }

                        // 处理拆分和编码
                        let radical_list = extract_radicals(radicals);
                        let code_list = extract_codes(codes, is_sun_moon);

                        if radical_list.len() == code_list.len() {
                            let mut i = 0;
                            for (radical, code) in radical_list.iter().zip(code_list.iter()) {
                                if counting {
                                    // 统计字根出现次数
                                    *radical_counts.entry(radical.to_string()).or_insert(0) += 1;
                                }

                                i += 1;
                                if i < 4 && code_list.len() == i {
                                    if !radical_codes.contains_key(radical)
                                        || radical_codes.get(radical).map_or("", |v| v).len()
                                            < code.len()
                                    {
                                        // 记录字根编码
                                        radical_codes.insert(radical.to_string(), code.to_string());
                                        // 特殊处理"曾中"字根，使用"横日"的编码
                                        if radical == "{横日}" {
                                            radical_codes
                                                .insert("{曾中}".to_string(), code.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 按编码排序并写入编码文件
    let mut sorted_codes: Vec<(&String, &String)> = radical_codes.iter().collect();
    sorted_codes.sort_by(|a, b| a.1.cmp(b.1));

    let mut code_file = File::create(code_output_path)?;
    for (radical, code) in sorted_codes {
        writeln!(code_file, "{} {}", code.trim(), radical.trim())?;
    }

    // 按频率排序并写入频率文件
    let mut sorted_counts: Vec<(&String, &u32)> = radical_counts.iter().collect();
    sorted_counts.sort_by(|a, b| b.1.cmp(a.1));

    let mut counts_file = File::create(counts_output_path)?;
    for (radical, count) in sorted_counts {
        writeln!(counts_file, "{} {}", radical.trim(), count)?;
    }

    Ok(())
}

fn extract_radicals(radicals: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_brackets = false;

    for c in radicals.chars() {
        match c {
            '{' => {
                in_brackets = true;
                current.clear();
                current.push(c);
            }
            '}' => {
                in_brackets = false;
                if !current.is_empty() {
                    current.push(c);
                    result.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                if in_brackets {
                    current.push(c);
                } else {
                    result.push(c.to_string());
                }
            }
        }
    }

    result
}

fn extract_codes(codes: &str, is_sun_moon: bool) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_code = false;

    for c in codes.chars() {
        if c.is_uppercase() {
            // 大写字母开始新编码
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
            }
            current.push(c.to_ascii_lowercase());
            in_code = true;
        } else if c.is_lowercase() && in_code {
            // 小写字母继续当前编码
            if c.is_ascii() {
                current.push(c);
            } else {
                current.push(char::from_u32((c as u32) - 9327).unwrap())
            }
        } else {
            // 其他字符结束当前编码
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
                in_code = false;
            }
        }
    }

    // 添加最后一个编码
    if !current.is_empty() {
        result.push(current);
    }

    // 如果编码长度>2，只取前两个字母
    result
        .iter()
        .map(|code| {
            if !is_sun_moon && code.len() > 2 {
                code[..2].to_string()
            } else {
                code.clone()
            }
        })
        .collect()
}

fn show_message(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    message: &str,
) -> Result<()> {
    terminal.draw(|f| {
        let size = f.area();
        let block = Block::default().title("提示").borders(Borders::ALL);
        let paragraph = Paragraph::new(message)
            .block(block)
            .alignment(Alignment::Center);
        f.render_widget(paragraph, size);
    })?;
    // 等待用户按键
    loop {
        if let Event::Key(key) = event::read()? {
            #[cfg(windows)]
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            if key.code == KeyCode::Enter {
                break;
            }
        }
    }
    Ok(())
}
