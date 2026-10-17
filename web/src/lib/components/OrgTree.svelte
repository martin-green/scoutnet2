<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, troopTypes, type Body, type MemberRow, type OrgNode, type OrgNodePage } from '#lib/api/client.ts';
	import { bodyHref, kindPlural, kindsBelow, kindTag } from '#lib/bodies.ts';
	import PersonPanel from './PersonPanel.svelte';

	let { root }: { root: Body } = $props();

	const canMembers = $derived(root.permissions.includes('members_read'));
	const below = $derived(kindsBelow(root.kind));
	const personKinds = ['group', 'troop', 'patrol'];

	// ---- Trädet: enheter laddas nivåvis, personer per kår (ett anrop) och fördelas på avdelning/patrull.
	let nodes = $state<Record<string, OrgNode>>({});
	let kids = $state<Record<string, string[]>>({});
	let loaded = $state<Record<string, boolean>>({});
	let expanded = $state<Record<string, boolean>>({});
	let busy = $state<Record<string, boolean>>({});
	let members = $state<Record<string, MemberRow[]>>({});
	let truncated = $state<Record<string, boolean>>({});
	let rootDepth = $state(0);
	let level = $state<string | null>(null);
	let error = $state<string | null>(null);
	let selectedPerson = $state<string | null>(null);

	function insert(items: OrgNode[]) {
		for (const n of items) {
			nodes[n.id] = n;
			const parent = n.parent_id ?? '';
			// Läs tillbaka via $state-proxyn; `??=` returnerar den råa arrayen och ändringar i den syns inte.
			if (!kids[parent]) kids[parent] = [];
			const list = kids[parent];
			if (!list.includes(n.id)) list.push(n.id);
		}
	}

	async function loadBelow(id: string, depth: number) {
		busy[id] = true;
		try {
			const res = await api.descendants(id, new URLSearchParams({ depth: String(depth), limit: '5000' }));
			insert(res.items);
			loaded[id] = true;
			for (const n of res.items) if (n.depth < depth) loaded[n.id] = true;
			if (id === root.id) rootDepth = Math.max(rootDepth, depth);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy[id] = false;
		}
	}

	/** Närmaste kår (eller roten) – personerna hämtas en gång per kår. */
	function personSource(id: string): string {
		let cur: string | null | undefined = id;
		while (cur && cur !== root.id) {
			const n: OrgNode | undefined = nodes[cur];
			if (!n) break;
			if (n.kind === 'group') return n.id;
			cur = n.parent_id;
		}
		return root.id;
	}

	async function ensurePersons(src: string) {
		if (members[src] || busy[`m:${src}`]) return;
		busy[`m:${src}`] = true;
		try {
			const res = await api.members(src, new URLSearchParams({ limit: '500', sort: 'name' }));
			members[src] = res.items;
			truncated[src] = res.total > res.items.length;
		} catch {
			members[src] = [];
		} finally {
			busy[`m:${src}`] = false;
		}
	}

	// Varje person hamnar under sin patrull, annars sin avdelning, annars kåren. Ledare först.
	const personsUnder = $derived.by(() => {
		const out: Record<string, MemberRow[]> = {};
		for (const [src, rows] of Object.entries(members)) {
			for (const r of rows) {
				const target =
					[r.patrol?.id, r.troop?.id].find((id) => id && (nodes[id] || id === root.id)) ?? src;
				(out[target] ??= []).push(r);
			}
		}
		for (const list of Object.values(out)) list.sort((a, b) => Number(b.is_leader) - Number(a.is_leader));
		return out;
	});

	const expandable = (n: OrgNode) =>
		n.has_children || (canMembers && personKinds.includes(n.kind) && n.counts.scouts + n.counts.leaders > 0);

	async function toggle(n: OrgNode, open = !expanded[n.id]) {
		level = null;
		expanded[n.id] = open;
		if (!open) return;
		if (n.has_children && !loaded[n.id]) await loadBelow(n.id, 2);
		if (canMembers && personKinds.includes(n.kind)) ensurePersons(personSource(n.id));
	}

	function onToggleKey(e: KeyboardEvent, n: OrgNode) {
		if (e.key === 'ArrowRight' && !expanded[n.id]) toggle(n, true);
		if (e.key === 'ArrowLeft' && expanded[n.id]) toggle(n, false);
	}

	// "Visa ned till": fäll ut alla enheter ovanför vald nivå.
	const levels = $derived([
		...below.map((k) => ({ key: k as string, label: kindPlural[k] })),
		...(canMembers && personKinds.includes(root.kind) ? [{ key: 'person', label: 'Personer' }] : [])
	]);

	async function showDownTo(key: string) {
		const depth = key === 'person' ? below.length : below.indexOf(key as never) + 1;
		if (depth > rootDepth) await loadBelow(root.id, depth);
		const stop = key === 'person' ? Infinity : below.indexOf(key as never);
		const next: Record<string, boolean> = {};
		for (const n of Object.values(nodes)) {
			const i = below.indexOf(n.kind as never);
			next[n.id] = key === 'person' ? expandable(n) : i >= 0 && i < stop;
		}
		expanded = next;
		level = key;
		if (key === 'person') ensurePersons(root.id);
	}

	onMount(() => {
		loadBelow(root.id, Math.min(2, Math.max(1, below.length)));
		if (canMembers && personKinds.includes(root.kind)) ensurePersons(root.id);
	});

	type Row =
		| { type: 'body'; node: OrgNode; level: number }
		| { type: 'person'; m: MemberRow; level: number }
		| { type: 'note'; text: string; level: number; key: string };

	const rows = $derived.by(() => {
		const out: Row[] = [];
		const walk = (id: string, lvl: number) => {
			for (const cid of kids[id] ?? []) {
				const n = nodes[cid];
				out.push({ type: 'body', node: n, level: lvl });
				if (expanded[cid]) walk(cid, lvl + 1);
			}
			if (canMembers) {
				for (const m of personsUnder[id] ?? []) out.push({ type: 'person', m, level: lvl });
				const src = personSource(id);
				if (busy[id] || (personKinds.includes(nodes[id]?.kind ?? root.kind) && busy[`m:${src}`])) {
					out.push({ type: 'note', text: 'Laddar…', level: lvl, key: `l:${id}` });
				} else if (src === id && truncated[id]) {
					out.push({ type: 'note', text: 'Visar de första 500 personerna – använd medlemslistan för fler.', level: lvl, key: `t:${id}` });
				}
			} else if (busy[id]) {
				out.push({ type: 'note', text: 'Laddar…', level: lvl, key: `l:${id}` });
			}
		};
		walk(root.id, 2);
		return out;
	});

	// ---- Listvy och sökning: platta, sorterbara listor över alla enheter under roten.
	const params = $derived(page.url.searchParams);
	const listMode = $derived(params.get('visa') === 'lista' || !!params.get('q'));
	const typ = $derived(params.get('typ') ?? '');
	const gren = $derived(params.get('gren') ?? '');
	const sort = $derived(params.get('sort') ?? 'name');

	function setParams(changes: Record<string, string | null>) {
		const u = new URL(page.url.href);
		for (const [k, v] of Object.entries(changes)) {
			if (v === null || v === '') u.searchParams.delete(k);
			else u.searchParams.set(k, v);
		}
		goto(u, { replace: true, reset: false });
	}

	let q = $state(page.url.searchParams.get('q') ?? '');
	$effect(() => {
		const value = q;
		const t = setTimeout(() => {
			if (value !== (page.url.searchParams.get('q') ?? '')) setParams({ q: value });
		}, 250);
		return () => clearTimeout(t);
	});

	let list = $state<OrgNodePage | null>(null);
	let listLoading = $state(false);
	let seq = 0;
	$effect(() => {
		if (!listMode) return;
		const query = new URLSearchParams({ limit: '500', sort });
		const text = params.get('q');
		if (text) query.set('q', text);
		if (typ) query.set('kind', typ);
		if (gren && typ === 'troop') query.set('troop_type', gren);
		const mine = ++seq;
		listLoading = true;
		api.descendants(root.id, query)
			.then((res) => {
				if (mine === seq) list = res;
			})
			.catch((e) => (error = e.message))
			.finally(() => {
				if (mine === seq) listLoading = false;
			});
	});

	function setMode(mode: 'tree' | 'list') {
		if (mode === 'tree') {
			q = '';
			setParams({ visa: null, q: null, typ: null, gren: null, sort: null });
		} else {
			setParams({ visa: 'lista', typ: typ || below[0] || null });
		}
	}

	type SortCol = { key: string; label: string; right?: boolean };
	const listCols: SortCol[] = [
		{ key: 'name', label: 'Namn' },
		{ key: '', label: 'Typ' },
		{ key: 'number', label: 'Nr', right: true },
		{ key: '', label: 'Tillhör' },
		{ key: '', label: 'Kommun' },
		{ key: 'scouts', label: 'Scouter', right: true },
		{ key: 'leaders', label: 'Ledare', right: true },
		{ key: 'waiting', label: 'Väntelista', right: true },
		{ key: 'pending', label: 'Ej godkända', right: true }
	];
	function sortBy(key: string) {
		if (!key) return;
		const numeric = key !== 'name';
		const current = sort.replace(/^-/, '');
		const next = current === key ? (sort.startsWith('-') ? key : `-${key}`) : numeric ? `-${key}` : key;
		setParams({ sort: next });
	}
	const sortMark = (key: string) => (sort.replace(/^-/, '') === key ? (sort.startsWith('-') ? '▼' : '▲') : '');
	const totals = $derived(
		list && list.total === list.items.length
			? list.items.reduce(
					(s, n) => ({
						scouts: s.scouts + n.counts.scouts,
						leaders: s.leaders + n.counts.leaders,
						waiting: s.waiting + n.counts.waiting,
						pending: s.pending + n.counts.pending
					}),
					{ scouts: 0, leaders: 0, waiting: 0, pending: 0 }
				)
			: null
	);

	const typeLabel = (n: { kind: string; kind_label: string; troop_type_label?: string | null }) =>
		n.troop_type_label ?? n.kind_label;
	const rootScouts = $derived(root.counts.active - root.counts.leaders);
</script>

<div class="card wrap">
	<div class="toolbar">
		<input class="input search" type="search" placeholder="Sök enhet – namn eller nummer…" bind:value={q} aria-label="Sök enhet" />
		<div class="seg" role="group" aria-label="Visning">
			<button class="chip" class:on={!listMode} onclick={() => setMode('tree')}>🌳 Träd</button>
			<button class="chip" class:on={listMode} onclick={() => setMode('list')}>☰ Lista</button>
		</div>
		{#if !listMode && levels.length > 0}
			<div class="seg" role="group" aria-label="Visa ned till">
				<span class="muted small">Visa ned till</span>
				{#each levels as l (l.key)}
					<button class="chip" class:on={level === l.key} onclick={() => showDownTo(l.key)}>{l.label}</button>
				{/each}
			</div>
		{/if}
	</div>

	{#if listMode}
		<div class="chips">
			<button class="chip" class:on={!typ} onclick={() => setParams({ visa: 'lista', typ: null, gren: null })}>Alla enheter</button>
			{#each below as k (k)}
				<button class="chip" class:on={typ === k} onclick={() => setParams({ visa: 'lista', typ: k, gren: k === 'troop' ? gren : null })}>
					{kindPlural[k]}
				</button>
			{/each}
			{#if typ === 'troop'}
				<span class="sep" aria-hidden="true"></span>
				{#each troopTypes as t (t.key)}
					<button class="chip" class:on={gren.split(',').includes(t.key)} onclick={() => {
						const cur = gren.split(',').filter(Boolean);
						setParams({ gren: (cur.includes(t.key) ? cur.filter((x) => x !== t.key) : [...cur, t.key]).join(',') });
					}}>{t.label}</button>
				{/each}
			{/if}
		</div>
	{/if}

	{#if error}<p class="notice error">Kunde inte hämta: {error}</p>{/if}

	{#if listMode}
		<div class="table-scroll" class:loading={listLoading}>
			<table>
				<thead>
					<tr>
						{#each listCols as c (c.label)}
							<th class:right={c.right} aria-sort={sortMark(c.key) ? (sortMark(c.key) === '▲' ? 'ascending' : 'descending') : undefined}>
								{#if c.key}
									<button class="th-btn" onclick={() => sortBy(c.key)}>{c.label} <span class="sort">{sortMark(c.key)}</span></button>
								{:else}
									{c.label}
								{/if}
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each list?.items ?? [] as n (n.id)}
						<tr>
							<td><span class="tag k-{n.kind}" title={n.kind_label}>{kindTag[n.kind] ?? '?'}</span> <a href={bodyHref(n)}>{n.name}</a></td>
							<td class="muted">{typeLabel(n)}</td>
							<td class="right mono">{n.number}</td>
							<td class="muted small">{n.path.join(' › ')}</td>
							<td class="muted">{n.municipality ?? ''}</td>
							<td class="right"><a href={`${bodyHref(n, 'medlemmar')}?leaders=false`}>{n.counts.scouts}</a></td>
							<td class="right"><a href={`${bodyHref(n, 'medlemmar')}?leaders=true`}>{n.counts.leaders}</a></td>
							<td class="right">{n.counts.waiting || ''}</td>
							<td class="right">{n.counts.pending || ''}</td>
						</tr>
					{:else}
						{#if !listLoading}
							<tr>
								<td colspan={listCols.length} class="empty">
									Inga enheter matchar.
									{#if typ || gren}<button class="link" onclick={() => setParams({ typ: null, gren: null })}>Sök bland alla enhetstyper</button>{/if}
								</td>
							</tr>
						{/if}
					{/each}
				</tbody>
				{#if totals && list && list.items.length > 1}
					<tfoot>
						<tr>
							<td colspan="5">Summa för {list.items.length} enheter</td>
							<td class="right">{totals.scouts}</td>
							<td class="right">{totals.leaders}</td>
							<td class="right">{totals.waiting}</td>
							<td class="right">{totals.pending}</td>
						</tr>
					</tfoot>
				{/if}
			</table>
		</div>
		{#if list}
			<p class="footer muted small">
				{list.total} enheter{list.total > list.items.length ? `, visar de första ${list.items.length}` : ''} · server {list.took_ms.toFixed(1)} ms
			</p>
		{/if}
	{:else}
		<div class="table-scroll">
			<table aria-label={`Organisationen under ${root.name}`}>
				<thead>
					<tr>
						<th>Namn</th>
						<th>Typ</th>
						<th class="right">Nr</th>
						<th class="right">Scouter</th>
						<th class="right">Ledare</th>
						<th class="right">Väntelista</th>
					</tr>
				</thead>
				<tbody>
					<tr class="root">
						<td>
							<span class="indent" style:--lvl={0}></span>
							<span class="toggle placeholder" aria-hidden="true">▾</span>
							<span class="tag k-{root.kind}">{kindTag[root.kind] ?? '?'}</span>
							<strong>{root.name}</strong>
						</td>
						<td class="muted">{typeLabel(root)}</td>
						<td class="right mono">{root.number}</td>
						<td class="right">{rootScouts}</td>
						<td class="right">{root.counts.leaders}</td>
						<td class="right">{root.counts.waiting || ''}</td>
					</tr>
					{#each rows as r (r.type === 'body' ? r.node.id : r.type === 'person' ? `p:${r.m.id}` : r.key)}
						{#if r.type === 'body'}
							{@const n = r.node}
							{@const canOpen = expandable(n)}
							<tr>
								<td>
									<span class="indent" style:--lvl={r.level - 1}></span>
									{#if canOpen}
										<button
											class="toggle"
											aria-expanded={!!expanded[n.id]}
											aria-label={`${expanded[n.id] ? 'Fäll ihop' : 'Visa innehållet i'} ${n.name}`}
											onclick={() => toggle(n)}
											onkeydown={(e) => onToggleKey(e, n)}>{expanded[n.id] ? '▾' : '▸'}</button>
									{:else}
										<span class="toggle placeholder" aria-hidden="true"></span>
									{/if}
									<span class="tag k-{n.kind}" title={n.kind_label}>{kindTag[n.kind] ?? '?'}</span>
									<a href={bodyHref(n)}>{n.name}</a>
								</td>
								<td class="muted">{typeLabel(n)}</td>
								<td class="right mono">{n.number}</td>
								<td class="right"><a class="count" href={`${bodyHref(n, 'medlemmar')}?leaders=false`}>{n.counts.scouts}</a></td>
								<td class="right"><a class="count" href={`${bodyHref(n, 'medlemmar')}?leaders=true`}>{n.counts.leaders}</a></td>
								<td class="right">
									{#if n.counts.waiting > 0 && n.kind === 'group'}<a class="count" href={bodyHref(n, 'vantelista')}>{n.counts.waiting}</a>{:else if n.counts.waiting > 0}{n.counts.waiting}{/if}
								</td>
							</tr>
						{:else if r.type === 'person'}
							<tr class="person">
								<td>
									<span class="indent" style:--lvl={r.level - 1}></span>
									<span class="toggle placeholder" aria-hidden="true"></span>
									<span class="tag person" aria-hidden="true">👤</span>
									<button class="link" onclick={() => (selectedPerson = r.m.id)}>{r.m.first_name} {r.m.last_name}</button>
								</td>
								<td>{#if r.m.is_leader}<span class="badge accent">Ledare</span>{:else}<span class="muted">Scout, {r.m.age} år</span>{/if}</td>
								<td class="right mono muted">{r.m.member_no}</td>
								<td colspan="3"></td>
							</tr>
						{:else}
							<tr class="note">
								<td colspan="6"><span class="indent" style:--lvl={r.level - 1}></span><span class="muted small">{r.text}</span></td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		</div>
		<p class="footer muted small">
			Antalen gäller allt under varje enhet. Klicka på ▸ eller använd piltangenterna för att fälla ut.
			{#if canMembers}Personer visas när du fäller ut en kår, avdelning eller patrull.{/if}
		</p>
	{/if}
</div>

{#if selectedPerson}
	<PersonPanel id={selectedPerson} onclose={() => (selectedPerson = null)} onopen={(pid) => (selectedPerson = pid)} />
{/if}

<style>
	.wrap {
		padding: 0;
		overflow: hidden;
	}
	.toolbar,
	.chips {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4) 0;
	}
	.chips {
		padding-top: var(--space-2);
	}
	.search {
		flex: 1 1 260px;
		max-width: 380px;
	}
	.seg {
		display: flex;
		align-items: center;
		gap: 4px;
		flex-wrap: wrap;
	}
	.sep {
		width: 1px;
		height: 20px;
		background: var(--border);
		margin: 0 4px;
	}
	.notice {
		margin: var(--space-3) var(--space-4) 0;
	}
	.table-scroll {
		overflow-x: auto;
		margin-top: var(--space-3);
		border-top: 1px solid var(--border);
	}
	.table-scroll.loading {
		opacity: 0.6;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.92rem;
	}
	th {
		text-align: left;
		font-weight: 600;
		color: var(--muted);
		background: var(--sn-gray-100);
		padding: 8px 10px;
		white-space: nowrap;
		position: sticky;
		top: 0;
	}
	td {
		padding: 5px 10px;
		border-bottom: 1px solid var(--border);
		white-space: nowrap;
	}
	tbody tr:hover {
		background: var(--sn-blue-50);
	}
	tr.root td {
		background: var(--sn-gray-100);
	}
	tfoot td {
		font-weight: 600;
		border-top: 2px solid var(--border);
	}
	.right {
		text-align: right;
	}
	.small {
		font-size: 0.82rem;
	}
	.indent {
		display: inline-block;
		width: calc(var(--lvl) * 22px);
	}
	.toggle {
		display: inline-grid;
		place-items: center;
		width: 22px;
		height: 22px;
		border: 0;
		background: none;
		color: var(--muted);
		cursor: pointer;
		border-radius: 4px;
		font-size: 0.9rem;
		vertical-align: middle;
	}
	.toggle:hover {
		background: var(--sn-blue-100);
		color: var(--primary-strong);
	}
	.toggle.placeholder {
		cursor: default;
	}
	.toggle.placeholder:hover {
		background: none;
	}
	.tag {
		display: inline-grid;
		place-items: center;
		min-width: 22px;
		height: 20px;
		padding: 0 3px;
		border-radius: 4px;
		font-size: 0.72rem;
		font-weight: 700;
		margin-right: 6px;
		background: var(--sn-gray-100);
		color: var(--badge-gray-text);
		vertical-align: middle;
	}
	.tag.k-organisation,
	.tag.k-district {
		background: var(--sn-blue-50);
		color: var(--primary-strong);
	}
	.tag.k-group {
		background: var(--sn-accent-50);
		color: var(--badge-accent-text);
	}
	.tag.k-troop {
		background: var(--sn-green-50);
		color: var(--badge-green-text);
	}
	.tag.person {
		background: none;
		font-size: 0.85rem;
	}
	a {
		color: var(--primary);
		text-decoration: none;
	}
	a:hover {
		text-decoration: underline;
	}
	a.count {
		color: inherit;
	}
	.link {
		border: 0;
		background: none;
		padding: 0;
		font: inherit;
		color: var(--primary);
		cursor: pointer;
	}
	.link:hover {
		text-decoration: underline;
	}
	.th-btn {
		border: 0;
		background: none;
		font: inherit;
		color: inherit;
		cursor: pointer;
		padding: 0;
	}
	.sort {
		color: var(--primary);
	}
	.empty {
		text-align: center;
		color: var(--muted);
		padding: var(--space-5);
	}
	.footer {
		padding: var(--space-2) var(--space-4) var(--space-3);
		margin: 0;
	}
</style>
