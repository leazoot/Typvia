// Principal class of the Typvia keyboard extension. The keyboard is a
// snippet PANEL, never a QWERTY: a caret-led search header, title+preview
// rows, and a fixed five-tab bar whose ABC key is the system input-mode
// switcher. Tap inserts immediately and the keyboard stays open; hold
// previews the body in-panel; templates with variables open an in-panel
// fill face before inserting.
//
// Security posture: RequestsOpenAccess=false; the App Group snapshot is
// read-only; no networking, no key logging, no host-app detection; sensitive
// entries render as locked dot rows with zero plaintext, tapping them does
// nothing, and the Vault tab is a locked state with no unlock control (the
// keyboard process can never decrypt).

import UIKit

class KeyboardViewController: UIInputViewController {
    private var store: SnapshotStore?
    private var activeTab: PanelTab = .recent
    private var openFolder: SnapshotFolder?
    /// Template whose fill face is open; always a normal template entry
    /// (the FFI rejects sensitive ids, and the UI never routes them here).
    private var openTemplate: SnapshotEntry?
    private var query = ""
    private var rows: [SnapshotEntry] = []
    private var folderRows: [SnapshotFolder] = []
    /// First body line per entry id, resolved lazily for row previews and
    /// dropped whenever the snapshot reloads. Sensitive ids resolve to nil
    /// by construction (the FFI returns no body for them).
    private var previewCache: [String: String?] = [:]

    private enum ListMode { case entries, folders }
    private var listMode: ListMode = .entries

    private let header = UIView()
    private let headerRule = UIView()
    private let caretBar = UIView()
    private let searchField = UITextField()
    private let backButton = UIButton(type: .custom)
    private let headerTitle = UILabel()
    private let headerRight = UILabel()
    private let table = UITableView()
    private let emptyState = EmptyStateView()
    private let vaultLocked = VaultLockedView()
    private let fillView = TemplateFillView()
    private let tabBar = PanelTabBar()
    private let insertedHint = InsertedHintView()
    private var previewOverlay: UIView?
    private var heightConstraint: NSLayoutConstraint?
    private var hintTimer: Timer?

    // MARK: Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = PanelTheme.panel
        view.accessibilityIdentifier = "kbPanel"
        buildLayout()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if heightConstraint == nil {
            // The panel model is height-capped so the host app stays visible
            // above; 999 priority yields to the system's own constraints.
            let constraint = view.heightAnchor.constraint(
                equalToConstant: PanelTheme.panelHeight)
            constraint.priority = UILayoutPriority(999)
            constraint.isActive = true
            heightConstraint = constraint
        }
        reloadSnapshot()
    }

    // MARK: Data

    private func reloadSnapshot() {
        store = SnapshotStore.load()
        previewCache.removeAll()
        if let folder = openFolder,
            store?.folders.contains(where: { $0.id == folder.id }) != true
        {
            openFolder = nil
        }
        if let template = openTemplate,
            store?.entries.contains(where: { $0.id == template.id }) != true
        {
            openTemplate = nil
        }
        applyFilters()
    }

    private func applyFilters() {
        let base: [SnapshotEntry]
        if let store {
            base = query.isEmpty ? store.entries : store.search(query)
        } else {
            base = []
        }
        listMode = .entries
        switch activeTab {
        case .recent:
            // The default view: every entry, recent first (FFI ordering).
            rows = base
        case .favorites:
            rows = base.filter(\.isFavorite)
        case .templates:
            rows = base.filter { $0.snippetType == "template" }
        case .folders:
            if let folder = openFolder {
                rows = (store?.entries ?? []).filter { $0.folderId == folder.id }
            } else {
                listMode = .folders
                folderRows = store?.folders ?? []
                rows = []
            }
        case .vault:
            // The vault never lists content in the keyboard; the tab is a
            // locked state.
            rows = []
        }
        table.reloadData()
        updateHeader()
        updateStateViews()
    }

    private func previewLine(for entry: SnapshotEntry) -> String? {
        if entry.isSensitive { return nil }
        if let cached = previewCache[entry.id] { return cached }
        let line = store?.body(for: entry.id)
            .flatMap { $0.components(separatedBy: "\n").first }
        previewCache[entry.id] = line
        return line
    }

    // MARK: Actions

    @objc private func searchChanged() {
        query = searchField.text ?? ""
        // Every text change replaces results immediately: no debounce, no
        // transition (design rule for search).
        applyFilters()
    }

    private func tabSelected(_ next: PanelTab) {
        activeTab = next
        openFolder = nil
        openTemplate = nil
        query = ""
        searchField.text = nil
        tabBar.configure(selected: next)
        applyFilters()
    }

    @objc private func backTapped() {
        if openTemplate != nil {
            openTemplate = nil
        } else {
            openFolder = nil
        }
        applyFilters()
    }

    private func insert(_ entry: SnapshotEntry) {
        // Locked rows never insert: body(for:) is nil for sensitive ids by
        // construction (the FFI returns nothing for them).
        guard !entry.isSensitive, let body = store?.body(for: entry.id) else { return }
        textDocumentProxy.insertText(body)
        showInsertedHint()
    }

    // MARK: Template fill

    private func enterTemplateFill(_ entry: SnapshotEntry, variables: [String]) {
        openTemplate = entry
        fillView.configure(variables: variables)
        // Copy needs the shared pasteboard, which iOS withholds from
        // keyboards without full access; hide it rather than show a dead
        // control.
        fillView.setCopyVisible(hasFullAccess)
        updateTemplatePreview()
        applyFilters()
    }

    private func updateTemplatePreview() {
        guard let entry = openTemplate else { return }
        let values = fillView.values
        let text = store?.fillPreview(id: entry.id, values: values) ?? ""
        fillView.setPreview(
            Self.previewAttributed(text, filledValues: Array(values.values)))
    }

    private func insertTemplate() {
        guard
            let entry = openTemplate,
            let rendered = store?.fillRender(id: entry.id, values: fillView.values)
        else { return }
        textDocumentProxy.insertText(rendered)
        showInsertedHint()
    }

    private func copyTemplate() {
        guard
            let entry = openTemplate,
            let rendered = store?.fillRender(id: entry.id, values: fillView.values)
        else { return }
        // Rendered normal-template text only; sensitive content cannot
        // reach this path (the FFI rejects sensitive ids structurally).
        UIPasteboard.general.string = rendered
        UIAccessibility.post(notification: .announcement, argument: "Copied")
    }

    /// Preview text with each filled value re-inked and underlined in a
    /// half-strength accent. Occurrences are matched textually,
    /// so a value that also appears in static text highlights there too.
    private static func previewAttributed(
        _ text: String, filledValues: [String]
    ) -> NSAttributedString {
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineHeightMultiple = 1.38
        let attributed = NSMutableAttributedString(
            string: text,
            attributes: [
                .font: PanelTheme.previewTextFont,
                .foregroundColor: PanelTheme.previewText,
                .paragraphStyle: paragraph,
            ])
        let whole = text as NSString
        for value in filledValues where !value.isEmpty {
            var search = NSRange(location: 0, length: whole.length)
            while search.length > 0 {
                let found = whole.range(of: value, options: [], range: search)
                if found.location == NSNotFound { break }
                attributed.addAttributes(
                    [
                        .foregroundColor: PanelTheme.ink,
                        .underlineStyle: NSUnderlineStyle.single.rawValue,
                        .underlineColor: PanelTheme.accent.withAlphaComponent(0.5),
                    ],
                    range: found)
                let next = found.location + found.length
                search = NSRange(location: next, length: whole.length - next)
            }
        }
        return attributed
    }

    @objc private func handleLongPress(_ gesture: UILongPressGestureRecognizer) {
        switch gesture.state {
        case .began:
            guard listMode == .entries else { return }
            let point = gesture.location(in: table)
            guard
                let indexPath = table.indexPathForRow(at: point),
                indexPath.row < rows.count
            else { return }
            let entry = rows[indexPath.row]
            guard !entry.isSensitive, let body = store?.body(for: entry.id) else { return }
            showPreview(body)
        case .ended, .cancelled, .failed:
            hidePreview()
        default:
            break
        }
    }

    // MARK: Inserted hint

    private func showInsertedHint() {
        hintTimer?.invalidate()
        UIAccessibility.post(notification: .announcement, argument: "Inserted")
        insertedHint.isHidden = false
        if UIAccessibility.isReduceMotionEnabled {
            insertedHint.alpha = 1
        } else {
            insertedHint.alpha = 0
            UIView.animate(withDuration: 0.15) { self.insertedHint.alpha = 1 }
        }
        hintTimer = Timer.scheduledTimer(withTimeInterval: 1.4, repeats: false) { [weak self] _ in
            self?.hideInsertedHint()
        }
    }

    private func hideInsertedHint() {
        if UIAccessibility.isReduceMotionEnabled {
            insertedHint.isHidden = true
            return
        }
        // Exit at 0.7 × the entrance duration (design motion gradient).
        UIView.animate(
            withDuration: 0.105,
            animations: { self.insertedHint.alpha = 0 },
            completion: { _ in self.insertedHint.isHidden = true })
    }

    // MARK: Preview (hold)

    private func showPreview(_ body: String) {
        hidePreview()
        let overlay = UIView()
        overlay.backgroundColor = PanelTheme.lifted
        overlay.layer.cornerRadius = 10
        overlay.layer.shadowColor = UIColor.black.cgColor
        overlay.layer.shadowOpacity = 0.09
        overlay.layer.shadowOffset = CGSize(width: 0, height: 3)
        overlay.layer.shadowRadius = 9
        overlay.translatesAutoresizingMaskIntoConstraints = false
        // The body is content on screen only while the finger holds; it is
        // hidden from accessibility like list previews elsewhere.
        overlay.accessibilityElementsHidden = true

        let text = UILabel()
        text.font = PanelTheme.previewBodyFont
        text.textColor = PanelTheme.ink
        text.numberOfLines = 8
        text.text = body
        text.translatesAutoresizingMaskIntoConstraints = false
        overlay.addSubview(text)
        view.addSubview(overlay)
        NSLayoutConstraint.activate([
            overlay.leadingAnchor.constraint(
                equalTo: view.leadingAnchor, constant: PanelTheme.hInset - 6),
            overlay.trailingAnchor.constraint(
                equalTo: view.trailingAnchor, constant: -(PanelTheme.hInset - 6)),
            overlay.topAnchor.constraint(equalTo: table.topAnchor, constant: 14),
            overlay.bottomAnchor.constraint(lessThanOrEqualTo: tabBar.topAnchor, constant: -8),
            text.topAnchor.constraint(equalTo: overlay.topAnchor, constant: 13),
            text.leadingAnchor.constraint(equalTo: overlay.leadingAnchor, constant: 14),
            text.trailingAnchor.constraint(equalTo: overlay.trailingAnchor, constant: -14),
            text.bottomAnchor.constraint(equalTo: overlay.bottomAnchor, constant: -13),
        ])
        previewOverlay = overlay
    }

    private func hidePreview() {
        previewOverlay?.removeFromSuperview()
        previewOverlay = nil
    }

    // MARK: Layout

    private func buildLayout() {
        // Header (48pt): brand caret bar + borderless search line, hairline
        // rule below that turns accent while a query is live. No box, no
        // magnifier icon. In-panel typing has no input source inside a
        // keyboard extension (the extension IS the keyboard), so text
        // reaches the field via paste or a hardware keyboard; the filter
        // pipeline reacts to any change.
        caretBar.backgroundColor = PanelTheme.accent
        searchField.font = PanelTheme.queryFont
        searchField.textColor = PanelTheme.ink
        searchField.tintColor = PanelTheme.accent
        searchField.borderStyle = .none
        searchField.autocorrectionType = .no
        searchField.accessibilityIdentifier = "kbSearch"
        searchField.addTarget(self, action: #selector(searchChanged), for: .editingChanged)
        headerRule.backgroundColor = PanelTheme.hairline

        backButton.setTitle("‹", for: .normal)
        backButton.titleLabel?.font = PanelTheme.backChevronFont
        backButton.setTitleColor(PanelTheme.preview, for: .normal)
        backButton.accessibilityLabel = "Back"
        backButton.accessibilityIdentifier = "kbBack"
        backButton.addTarget(self, action: #selector(backTapped), for: .touchUpInside)
        backButton.isHidden = true
        headerTitle.font = PanelTheme.headerTitleFont
        headerTitle.textColor = PanelTheme.ink
        headerTitle.isHidden = true
        headerRight.setContentCompressionResistancePriority(.required, for: .horizontal)

        table.dataSource = self
        table.delegate = self
        table.register(SnippetRowCell.self, forCellReuseIdentifier: SnippetRowCell.reuseId)
        table.rowHeight = PanelTheme.rowHeight
        table.separatorColor = PanelTheme.separator
        table.separatorInset = UIEdgeInsets(
            top: 0, left: PanelTheme.hInset, bottom: 0, right: PanelTheme.hInset)
        table.backgroundColor = .clear
        table.tableFooterView = UIView()
        table.accessibilityIdentifier = "kbList"
        let hold = UILongPressGestureRecognizer(
            target: self, action: #selector(handleLongPress(_:)))
        hold.minimumPressDuration = 0.45
        table.addGestureRecognizer(hold)

        tabBar.onSelect = { [weak self] tab in self?.tabSelected(tab) }
        tabBar.configure(selected: activeTab)
        // The ABC key is the system input-mode switcher; allTouchEvents lets
        // the system drive both tap-to-switch and hold-for-list.
        tabBar.abcKey.addTarget(
            self, action: #selector(handleInputModeList(from:with:)), for: .allTouchEvents)

        vaultLocked.isHidden = true
        insertedHint.isHidden = true
        fillView.isHidden = true
        fillView.onChange = { [weak self] in self?.updateTemplatePreview() }
        fillView.onInsert = { [weak self] in self?.insertTemplate() }
        fillView.onCopy = { [weak self] in self?.copyTemplate() }

        for view in [header, table, emptyState, vaultLocked, fillView, tabBar, insertedHint] {
            view.translatesAutoresizingMaskIntoConstraints = false
            self.view.addSubview(view)
        }
        for view in [headerRule, caretBar, searchField, backButton, headerTitle, headerRight] {
            view.translatesAutoresizingMaskIntoConstraints = false
            header.addSubview(view)
        }

        let inset = PanelTheme.hInset
        NSLayoutConstraint.activate([
            header.topAnchor.constraint(equalTo: view.topAnchor),
            header.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            header.heightAnchor.constraint(equalToConstant: PanelTheme.headerHeight),
            headerRule.leadingAnchor.constraint(equalTo: header.leadingAnchor),
            headerRule.trailingAnchor.constraint(equalTo: header.trailingAnchor),
            headerRule.bottomAnchor.constraint(equalTo: header.bottomAnchor),
            headerRule.heightAnchor.constraint(equalToConstant: 1),

            caretBar.leadingAnchor.constraint(equalTo: header.leadingAnchor, constant: inset),
            caretBar.centerYAnchor.constraint(equalTo: header.centerYAnchor),
            caretBar.widthAnchor.constraint(equalToConstant: 2),
            caretBar.heightAnchor.constraint(equalToConstant: 15),
            searchField.leadingAnchor.constraint(equalTo: caretBar.trailingAnchor, constant: 10),
            searchField.trailingAnchor.constraint(
                lessThanOrEqualTo: headerRight.leadingAnchor, constant: -12),
            searchField.centerYAnchor.constraint(equalTo: header.centerYAnchor),
            searchField.heightAnchor.constraint(equalToConstant: 28),

            backButton.leadingAnchor.constraint(equalTo: header.leadingAnchor, constant: inset),
            backButton.centerYAnchor.constraint(equalTo: header.centerYAnchor),
            backButton.widthAnchor.constraint(equalToConstant: 24),
            backButton.heightAnchor.constraint(equalTo: header.heightAnchor),
            headerTitle.leadingAnchor.constraint(equalTo: backButton.trailingAnchor, constant: 6),
            headerTitle.trailingAnchor.constraint(
                lessThanOrEqualTo: headerRight.leadingAnchor, constant: -12),
            headerTitle.centerYAnchor.constraint(equalTo: header.centerYAnchor),

            headerRight.trailingAnchor.constraint(
                equalTo: header.trailingAnchor, constant: -inset),
            headerRight.centerYAnchor.constraint(equalTo: header.centerYAnchor),

            table.topAnchor.constraint(equalTo: header.bottomAnchor),
            table.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            table.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            table.bottomAnchor.constraint(equalTo: tabBar.topAnchor),
            emptyState.topAnchor.constraint(equalTo: table.topAnchor),
            emptyState.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            emptyState.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            emptyState.bottomAnchor.constraint(equalTo: table.bottomAnchor),
            vaultLocked.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 56),
            vaultLocked.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            fillView.topAnchor.constraint(equalTo: header.bottomAnchor),
            fillView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            fillView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            fillView.bottomAnchor.constraint(equalTo: tabBar.topAnchor),

            tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            tabBar.bottomAnchor.constraint(
                equalTo: view.bottomAnchor, constant: -PanelTheme.bottomInset),
            tabBar.heightAnchor.constraint(equalToConstant: PanelTheme.tabBarHeight),

            insertedHint.leadingAnchor.constraint(
                equalTo: view.leadingAnchor, constant: inset),
            insertedHint.trailingAnchor.constraint(
                lessThanOrEqualTo: view.trailingAnchor, constant: -inset),
            insertedHint.bottomAnchor.constraint(equalTo: tabBar.topAnchor, constant: -16),
        ])
    }

    // MARK: Header + state rendering

    private func updateHeader() {
        let fillOpen = openTemplate != nil
        let folderOpen = activeTab == .folders && openFolder != nil
        let subviewOpen = fillOpen || folderOpen
        backButton.isHidden = !subviewOpen
        headerTitle.isHidden = !subviewOpen
        caretBar.isHidden = subviewOpen
        searchField.isHidden = subviewOpen
        headerTitle.text = fillOpen ? openTemplate?.title : openFolder?.name
        if fillOpen {
            // Template fill header: back + template title + TEMPLATE caps tag.
            headerRight.attributedText = NSAttributedString(
                string: "TEMPLATE",
                attributes: [
                    .font: PanelTheme.capsFont,
                    .foregroundColor: PanelTheme.rowMeta,
                    .kern: PanelTheme.capsKern,
                ])
            headerRule.backgroundColor = PanelTheme.hairline
            view.backgroundColor = PanelTheme.panel
            return
        }

        // Search lives on the entry-list tabs; Folders root and Vault show
        // the tab name on a dimmed caret instead.
        let searchable: Bool
        switch activeTab {
        case .recent, .favorites, .templates: searchable = true
        case .folders, .vault: searchable = false
        }
        searchField.isEnabled = searchable
        if !searchable { searchField.text = nil }
        caretBar.backgroundColor = searchable ? PanelTheme.accent : PanelTheme.caretDim
        searchField.attributedPlaceholder = NSAttributedString(
            string: searchable ? "Search snippets" : activeTab.label,
            attributes: [
                .font: PanelTheme.searchPlaceholderFont,
                .foregroundColor: PanelTheme.meta,
            ])

        let searching = searchable && !query.isEmpty
        headerRule.backgroundColor = searching ? PanelTheme.accent : PanelTheme.hairline
        if searching {
            headerRight.attributedText = NSAttributedString(
                string: "\(rows.count) found",
                attributes: [
                    .font: PanelTheme.countFont,
                    .foregroundColor: PanelTheme.rowMeta,
                ])
        } else {
            headerRight.attributedText = NSAttributedString(
                string: "TYPVIA",
                attributes: [
                    .font: PanelTheme.wordmarkFont,
                    .foregroundColor: PanelTheme.rowMeta,
                    .kern: PanelTheme.wordmarkKern,
                ])
        }
        view.backgroundColor = activeTab == .vault ? PanelTheme.vaultPanel : PanelTheme.panel
    }

    private func updateStateViews() {
        let fillOpen = openTemplate != nil
        fillView.isHidden = !fillOpen
        if fillOpen {
            table.isHidden = true
            emptyState.isHidden = true
            vaultLocked.isHidden = true
            return
        }
        if activeTab == .vault {
            table.isHidden = true
            let hasVaultEntries = store?.entries.contains(where: \.isSensitive) == true
            vaultLocked.isHidden = !hasVaultEntries
            emptyState.isHidden = hasVaultEntries
            if !hasVaultEntries {
                emptyState.show(
                    main: "No vault snippets yet",
                    sub: "还没有保险库片段")
            }
            return
        }
        vaultLocked.isHidden = true
        let hasRows = listMode == .folders ? !folderRows.isEmpty : !rows.isEmpty
        table.isHidden = !hasRows
        emptyState.isHidden = hasRows
        guard !hasRows else { return }
        let snapshotEmpty = store == nil || store?.entries.isEmpty == true
        if snapshotEmpty {
            // Covers missing/unreadable/newer-version snapshots too: the
            // honest answer to all of them is "nothing here yet".
            emptyState.show(
                main: "Open Typvia to save your first snippet",
                sub: "打开 Typvia 保存你的第一个片段")
        } else if !query.isEmpty {
            emptyState.show(
                main: "No matching snippets",
                sub: "没有匹配的片段")
        } else {
            switch activeTab {
            case .favorites:
                emptyState.show(main: "No favorites yet", sub: "还没有收藏")
            case .templates:
                emptyState.show(main: "No templates yet", sub: "还没有模板")
            case .folders:
                if openFolder == nil {
                    emptyState.show(main: "No folders yet", sub: "还没有文件夹")
                } else {
                    emptyState.show(
                        main: "No snippets in this folder", sub: "这个文件夹还没有片段")
                }
            case .recent, .vault:
                emptyState.show(main: "No snippets yet", sub: "还没有片段")
            }
        }
    }
}

// MARK: - Table

extension KeyboardViewController: UITableViewDataSource, UITableViewDelegate {
    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        listMode == .folders ? folderRows.count : rows.count
    }

    func tableView(
        _ tableView: UITableView, cellForRowAt indexPath: IndexPath
    ) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(
            withIdentifier: SnippetRowCell.reuseId, for: indexPath)
        guard let cell = cell as? SnippetRowCell else { return cell }
        switch listMode {
        case .folders:
            if indexPath.row < folderRows.count {
                cell.configure(folder: folderRows[indexPath.row])
            }
        case .entries:
            if indexPath.row < rows.count {
                let entry = rows[indexPath.row]
                cell.configure(
                    entry: entry,
                    previewLine: previewLine(for: entry),
                    highlight: query)
            }
        }
        return cell
    }

    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: false)
        switch listMode {
        case .folders:
            guard indexPath.row < folderRows.count else { return }
            openFolder = folderRows[indexPath.row]
            applyFilters()
        case .entries:
            guard indexPath.row < rows.count else { return }
            let entry = rows[indexPath.row]
            if activeTab == .templates, !entry.isSensitive {
                let variables = store?.variableNames(for: entry.id) ?? []
                if !variables.isEmpty {
                    enterTemplateFill(entry, variables: variables)
                    return
                }
            }
            insert(entry)
        }
    }
}
