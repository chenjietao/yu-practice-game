use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use game::{GameConfig, GameState, Radical, GameMode};
use std::io;
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

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

    // 尝试从多个可能的位置加载资源文件
    let mut counts_path = None;
    let mut radical_path = None;

    // 1. 首先尝试从可执行文件目录查找
    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            let exe_counts = parent.join("res/counts.txt");
            let exe_radical = parent.join(&config.radical_file);
            
            if exe_counts.exists() && exe_radical.exists() {
                counts_path = Some(exe_counts);
                radical_path = Some(exe_radical);
            }
        }
    }

    // 2. 如果可执行文件目录找不到，尝试从项目根目录查找
    if counts_path.is_none() || radical_path.is_none() {
        let project_counts = std::path::Path::new("res/counts.txt");
        let project_radical = std::path::Path::new(&config.radical_file);
        
        if project_counts.exists() && project_radical.exists() {
            counts_path = Some(project_counts.to_path_buf());
            radical_path = Some(project_radical.to_path_buf());
        }
    }

    // 检查是否找到有效的资源路径
    let (counts_path, radical_path) = match (counts_path, radical_path) {
        (Some(c), Some(r)) => (c, r),
        _ => return Err(anyhow::anyhow!(
            "无法找到资源文件，请确保res目录位于可执行文件目录或项目根目录下"
        )),
    };

    // 加载字根数据
    let radicals = Radical::load_from_files(
        counts_path.to_str().ok_or_else(|| anyhow::anyhow!("无效路径"))?, 
        radical_path.to_str().ok_or_else(|| anyhow::anyhow!("无效路径"))?
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
            let size = f.size();
            
            // 在摸鱼模式下，先绘制随机字符背景
            if config.mode == GameMode::Pretend {
                let pretend_text = Paragraph::new(game_state.generate_pretend_chars())
                    .block(Block::default());
                f.render_widget(pretend_text, size);
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),  // 当前字根
                    Constraint::Length(3),  // 输入框
                    Constraint::Length(3),  // 错误提示
                    Constraint::Min(3),  // 键盘布局
                    Constraint::Length(3),  // 统计信息
                ])
                .split(size);

            // 显示当前字根
            let border_style = match config.mode {
                GameMode::Normal => Borders::ALL,
                GameMode::Pretend => Borders::NONE,
            };

            if let Some(radical) = game_state.current_radical() {
                let radical_block = Block::default()
                    .title("当前字根")
                    .borders(border_style);
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
            let error_block = Block::default()
                .title("提示")
                .borders(border_style);
            let error_text = if let Some(error_msg) = &game_state.last_error {
                let style = if error_msg.starts_with("【正确】") {
                    tui::style::Style::default().fg(tui::style::Color::Green)
                } else {
                    tui::style::Style::default().fg(tui::style::Color::Red)
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
                use tui::text::{Span, Spans};
                use tui::style::{Style, Color};

                let keyboard_block = Block::default()
                    .borders(Borders::NONE);

                // 创建键盘布局行
                let mut rows = vec![];
                
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
                    row1.push(Span::styled(format!("⌈{}⌉", c), style));
                    row1.push(Span::raw(" "));
                }
                rows.push(Spans::from(row1));

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
                    row2.push(Span::styled(format!("⌈{}⌉", c), style));
                    row2.push(Span::raw(" "));
                }
                row2.push(Span::raw("  "));
                rows.push(Spans::from(row2));

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
                    row3.push(Span::styled(format!("⌈{}⌉", c), style));
                    row3.push(Span::raw(" "));
                }
                row3.push(Span::raw("       "));
                rows.push(Spans::from(row3));

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
                game_state.progress().0, game_state.progress().1,
                game_state.correct_count, game_state.wrong_count,
                quit_key
            );
            let stats_block = Block::default()
                .title("统计信息")
                .borders(border_style);
            let stats_text = Paragraph::new(stats)
                .block(stats_block);
            f.render_widget(stats_text, chunks[4]);
        })?;

        // 处理用户输入
        if let Event::Key(key) = event::read()? {
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
                KeyCode::Esc  => {
                    return Ok(());
                }
                _ => {}
            }
        }

        // 只在需要控制游戏节奏的地方添加延迟
    }
}

fn show_welcome(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    terminal.draw(|f| {
        let size = f.size();
        let block = Block::default()
            .title("宇浩字根练习")
            .borders(Borders::ALL);
        let welcome_text = Paragraph::new(
            "欢迎使用宇浩字根练习游戏!\n\n按任意键继续..."
        )
        .block(block)
        .alignment(Alignment::Center);
        f.render_widget(welcome_text, size);
    })?;
    event::read()?;
    Ok(())
}

fn show_message(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    message: &str,
) -> Result<()> {
    terminal.draw(|f| {
        let size = f.size();
        let block = Block::default()
            .title("提示")
            .borders(Borders::ALL);
        let paragraph = Paragraph::new(message)
            .block(block)
            .alignment(Alignment::Center);
        f.render_widget(paragraph, size);
    })?;
    // 等待用户按键
    while event::read()? != Event::Key(KeyCode::Enter.into()) {}
    Ok(())
}
