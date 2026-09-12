    use super::*;
    use super::sidebar::*;

    fn store_session(name: &str, cwd: &str) -> SessionInfo {
        SessionInfo {
            path: PathBuf::from(format!("/store/{name}.jsonl")),
            id: name.into(),
            cwd: PathBuf::from(cwd),
            title: format!("{name} title"),
            first_message: "preview".into(),
            modified: SystemTime::UNIX_EPOCH,
        }
    }

    fn session_row_paths(rows: &[SideRow], sessions: &[SessionInfo]) -> Vec<PathBuf> {
        rows.iter()
            .filter_map(|r| match r {
                SideRow::Session(ix) => Some(sessions[*ix].path.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn collapsed_workspace_pins_the_open_session() {
        // "alpha" is the working workspace (expanded by default); "beta" is
        // a foreign workspace, collapsed unless explicitly expanded. It
        // holds the open session: the group stays collapsed (siblings
        // hidden) but the open session's row stays pinned under the header.
        let sessions = vec![
            store_session("a1", "/work/alpha"),
            store_session("b1", "/work/beta"),
            store_session("b2", "/work/beta"),
        ];
        let active = Some(sessions[1].path.clone());
        let rows = build_sidebar_rows(
            &sessions,
            "alpha",
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &active,
            &HashSet::new(),
            false,
        );
        let visible = session_row_paths(&rows, &sessions);
        assert!(
            visible.contains(&sessions[1].path),
            "the open session stays visible in a collapsed workspace"
        );
        assert!(
            !visible.contains(&sessions[2].path),
            "its siblings stay hidden while the group is collapsed"
        );
        let beta_header = rows.iter().find_map(|r| match r {
            SideRow::Workspace {
                label, collapsed, ..
            } if label == "beta" => Some(*collapsed),
            _ => None,
        });
        assert_eq!(
            beta_header,
            Some(true),
            "the group header still reads as collapsed"
        );
    }

    #[test]
    fn collapsed_workspace_without_the_open_session_stays_closed() {
        let sessions = vec![
            store_session("a1", "/work/alpha"),
            store_session("b1", "/work/beta"),
        ];
        let active = Some(sessions[0].path.clone());
        let rows = build_sidebar_rows(
            &sessions,
            "alpha",
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &active,
            &HashSet::new(),
            false,
        );
        assert!(
            !session_row_paths(&rows, &sessions).contains(&sessions[1].path),
            "a collapsed foreign workspace keeps its sessions hidden"
        );
    }

    #[test]
    fn manually_collapsed_working_workspace_pins_the_open_session() {
        // The user collapsed their own workspace by hand — the open session
        // still shows under the header; expanding brings back the rest.
        let sessions = vec![
            store_session("a1", "/work/alpha"),
            store_session("a2", "/work/alpha"),
        ];
        let active = Some(sessions[0].path.clone());
        let mut collapsed = HashSet::new();
        collapsed.insert("alpha".to_string());
        let rows = build_sidebar_rows(
            &sessions,
            "alpha",
            &collapsed,
            &HashSet::new(),
            &HashSet::new(),
            &active,
            &HashSet::new(),
            false,
        );
        let visible = session_row_paths(&rows, &sessions);
        assert!(
            visible.contains(&sessions[0].path),
            "the open session stays visible even when its workspace was collapsed by hand"
        );
        assert!(
            !visible.contains(&sessions[1].path),
            "the other sessions stay hidden until the header is expanded"
        );
    }

    #[test]
    fn hidden_workspace_drops_its_group_and_sessions() {
        let sessions = vec![
            store_session("a1", "/work/alpha"),
            store_session("b1", "/work/beta"),
        ];
        let mut hidden = HashSet::new();
        hidden.insert("beta".to_string());
        let rows = build_sidebar_rows(
            &sessions,
            "alpha",
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &None,
            &hidden,
            false,
        );
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, SideRow::Workspace { label, .. } if label == "beta")),
            "a hidden workspace leaves no header behind"
        );
        assert!(
            !session_row_paths(&rows, &sessions).contains(&sessions[1].path),
            "its sessions leave the sidebar with it"
        );
        assert!(
            rows.iter().any(|r| matches!(
                r,
                SideRow::HiddenHeader { count, expanded } if *count == 1 && !*expanded
            )),
            "the Hidden disclosure stands in for the dropped group"
        );
    }

    #[test]
    fn expanded_hidden_section_lists_workspaces_for_restore() {
        let sessions = vec![store_session("b1", "/work/beta")];
        let mut hidden = HashSet::new();
        hidden.insert("beta".to_string());
        let rows = build_sidebar_rows(
            &sessions,
            "alpha",
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
            &None,
            &hidden,
            true,
        );
        assert!(
            rows.iter().any(|r| matches!(
                r,
                SideRow::HiddenWorkspace { label, count, .. } if label == "beta" && *count == 1
            )),
            "expanding the Hidden section lists each hidden workspace"
        );
        assert!(
            session_row_paths(&rows, &sessions).is_empty(),
            "restore is offered, but the sessions stay hidden until shown"
        );
    }
