<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		api,
		ApiError,
		feeStatusLabels,
		personStatusLabels,
		policeCheckLabels,
		redactedLabels,
		troopTypes,
		troopTypeLabel,
		type BodyChild,
		type MemberPage,
		type MemberRow,
		type SavedView
	} from '#lib/api/client.ts';
	import { SvelteSet } from 'svelte/reactivity';
	import BulkDialog from './BulkDialog.svelte';
	import PersonPanel from './PersonPanel.svelte';

	type Preset = 'members' | 'waiting';
	let {
		bodyId,
		preset = 'members',
		troops = [],
		canManage = false,
		modules = []
	}: { bodyId: string; preset?: Preset; troops?: BodyChild[]; canManage?: boolean; modules?: string[] } = $props();

	// `module`: kolumnen kommer från en modul och visas bara när modulen är på för enheten.
	type Col = { key: string; label: string; sort?: string; on: boolean; align?: 'right'; only?: Preset; module?: string };
	const allColumns: Col[] = [
		{ key: 'name', label: 'Namn', sort: 'name', on: true },
		{ key: 'member_no', label: 'Medl.nr', sort: 'member_no', on: false, align: 'right' },
		{ key: 'age', label: 'Ålder', sort: 'age', on: true, align: 'right' },
		{ key: 'birth_date', label: 'Född', sort: 'birth_date', on: false },
		{ key: 'troop', label: 'Avdelning', sort: 'troop', on: true, only: 'members' },
		{ key: 'patrol', label: 'Patrull', sort: 'patrol', on: true, only: 'members' },
		{ key: 'role', label: 'Roll', on: true, only: 'members' },
		{ key: 'person_status', label: 'Medlemsstatus', on: false },
		{ key: 'waiting_since', label: 'Väntat sedan', sort: 'waiting_since', on: true, only: 'waiting' },
		{ key: 'wished', label: 'Önskad gren', on: true, only: 'waiting' },
		{ key: 'email', label: 'E-post', on: true },
		{ key: 'alt_email', label: 'Alt. e-post', on: false },
		{ key: 'mobile_phone', label: 'Mobil', on: false },
		{ key: 'home_phone', label: 'Hemtelefon', on: false },
		{ key: 'national_id', label: 'Personnummer', on: false },
		{ key: 'guardians', label: 'Vårdnadshavare', on: true },
		{ key: 'attendance', label: 'Närvaro', sort: 'attendance', on: true, only: 'members', module: 'attendance' },
		{ key: 'police_check', label: 'Registerutdrag', sort: 'police_check', on: true, only: 'members' },
		{ key: 'fee', label: 'Terminsavgift', sort: 'fee', on: true, only: 'members' },
		{ key: 'fee_group', label: 'Avgiftsgrupp', on: false, only: 'members' },
		{ key: 'member_since', label: 'Medlem sedan', sort: 'member_since', on: false, only: 'members' }
	];
	const columns = $derived(
		allColumns.filter((c) => (!c.only || c.only === preset) && (!c.module || modules.includes(c.module)))
	);
	const storageKey = $derived(`sn2.columns.${preset}`);

	function initialVisible(): Record<string, boolean> {
		const defaults = Object.fromEntries(columns.map((c) => [c.key, c.on]));
		const saved = typeof localStorage !== 'undefined' ? localStorage.getItem(storageKey) : null;
		// Nya kolumner får sitt standardläge även om användaren har sparat ett eget urval.
		return saved ? { ...defaults, ...JSON.parse(saved) } : defaults;
	}
	let visible = $state<Record<string, boolean>>(initialVisible());
	$effect(() => localStorage.setItem(storageKey, JSON.stringify(visible)));
	const shown = $derived(columns.filter((c) => visible[c.key]));

	// URL är sanningen för filter, sortering och sida, så att vyer kan delas med en länk.
	const params = $derived(page.url.searchParams);
	const defaultSort = $derived(preset === 'waiting' ? 'waiting_since' : 'name');
	const sortKeys = $derived((params.get('sort') ?? defaultSort).split(',').filter(Boolean));
	const effective = $derived.by(() => {
		const p = new URLSearchParams(params.toString());
		if (preset === 'waiting') p.set('status', 'waiting');
		if (!p.has('sort')) p.set('sort', defaultSort);
		if (!p.has('limit')) p.set('limit', '100');
		return p;
	});

	let data = $state<MemberPage | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let selected = $state<string | null>(null);
	let reload = $state(0);
	let seq = 0;

	$effect(() => {
		const query = effective.toString();
		const id = bodyId;
		void reload;
		const mine = ++seq;
		loading = true;
		api.members(id, new URLSearchParams(query))
			.then((d) => {
				if (mine === seq) {
					data = d;
					error = null;
				}
			})
			.catch((e) => {
				if (mine === seq) error = e.message;
			})
			.finally(() => {
				if (mine === seq) loading = false;
			});
	});

	function setParams(changes: Record<string, string | null>) {
		const u = new URL(page.url.href);
		for (const [k, v] of Object.entries(changes)) {
			if (v === null || v === '') u.searchParams.delete(k);
			else u.searchParams.set(k, v);
		}
		if (!('offset' in changes)) u.searchParams.delete('offset');
		// Ett nytt filter ger ett nytt urval; byte av sida eller sortering behåller det.
		if (Object.keys(changes).some((k) => !['offset', 'limit', 'sort'].includes(k))) picked.clear();
		goto(u, { replace: true, reset: false });
	}

	// Urval för massåtgärder. Shift-klick markerar ett intervall på sidan.
	const picked = new SvelteSet<string>();
	let lastPicked = -1;
	let bulkAction = $state<'move' | 'admit' | null>(null);
	const pageIds = $derived((data?.items ?? []).map((r) => r.id));
	const allOnPage = $derived(pageIds.length > 0 && pageIds.every((id) => picked.has(id)));
	const someOnPage = $derived(pageIds.some((id) => picked.has(id)));
	function togglePick(i: number, e: MouseEvent) {
		const id = pageIds[i];
		const on = !picked.has(id);
		const [from, to] = e.shiftKey && lastPicked >= 0 ? [Math.min(lastPicked, i), Math.max(lastPicked, i)] : [i, i];
		for (const pid of pageIds.slice(from, to + 1)) {
			if (on) picked.add(pid);
			else picked.delete(pid);
		}
		lastPicked = i;
	}
	function togglePage() {
		const on = !allOnPage;
		for (const id of pageIds) {
			if (on) picked.add(id);
			else picked.delete(id);
		}
	}
	function bulkDone() {
		bulkAction = null;
		picked.clear();
		reload++;
	}

	// Fritextsök med fördröjning.
	let q = $state(page.url.searchParams.get('q') ?? '');
	$effect(() => {
		const value = q;
		const t = setTimeout(() => {
			if (value !== (page.url.searchParams.get('q') ?? '')) setParams({ q: value });
		}, 250);
		return () => clearTimeout(t);
	});

	const selectedTypes = $derived((params.get('troop_type') ?? '').split(',').filter(Boolean));
	function toggleType(key: string) {
		const next = selectedTypes.includes(key) ? selectedTypes.filter((t) => t !== key) : [...selectedTypes, key];
		setParams({ troop_type: next.join(',') });
	}

	function sortBy(col: Col, e: MouseEvent) {
		if (!col.sort) return;
		const current = sortKeys.find((k) => k.replace(/^-/, '') === col.sort);
		const toggled = current === col.sort ? `-${col.sort}` : col.sort;
		const next = e.shiftKey
			? current
				? sortKeys.map((k) => (k === current ? toggled : k))
				: [...sortKeys, col.sort]
			: [current === col.sort ? `-${col.sort}` : col.sort];
		setParams({ sort: next.join(',') });
	}
	function sortState(col: Col): { dir: '▲' | '▼'; n: number } | null {
		const i = sortKeys.findIndex((k) => k.replace(/^-/, '') === col.sort);
		return i < 0 ? null : { dir: sortKeys[i].startsWith('-') ? '▼' : '▲', n: i + 1 };
	}

	const filtersActive = $derived(
		['q', 'troop_type', 'troop_id', 'born_from', 'born_to', 'leaders', 'missing_guardian_email', 'police_check', 'fee_status'].some((k) =>
			params.has(k)
		)
	);
	function toggleChip(key: string, value: string) {
		setParams({ [key]: params.get(key) === value ? null : value });
	}
	// Filter som rollen saknar behörighet för döljs i stället för att alltid ge noll träffar.
	const canPolice = $derived(!data?.redacted_fields.includes('police_check'));
	const canFee = $derived(!data?.redacted_fields.includes('fee_status'));
	function clearFilters() {
		q = '';
		goto(page.url.pathname, { replace: true, reset: false });
	}

	// Sparade vyer på servern: egna, och delade med alla som ser listan i enheten.
	let views = $state<SavedView[]>([]);
	let showSave = $state(false);
	let viewName = $state('');
	let viewShared = $state(false);
	let viewError = $state<string | null>(null);
	$effect(() => {
		const id = bodyId;
		const list = preset;
		let current = true;
		api.views(id, list)
			.then((v) => current && (views = v))
			.catch(() => (views = []));
		return () => (current = false);
	});
	// Samma normalisering som servern gör, så att aktiv vy kan markeras.
	const normalize = (q: string) =>
		[...new URLSearchParams(q)]
			.filter(([k, v]) => v !== '' && k !== 'offset')
			.map(([k, v]) => `${k}=${v}`)
			.sort()
			.join('&');
	const activeQuery = $derived(normalize(params.toString()));
	async function saveView(e: SubmitEvent) {
		e.preventDefault();
		viewError = null;
		try {
			const saved = await api.createView(bodyId, {
				list: preset,
				name: viewName.trim(),
				query: params.toString(),
				shared: viewShared
			});
			views = [...views.filter((v) => v.id !== saved.id && !(v.mine && v.name.toLowerCase() === saved.name.toLowerCase())), saved].sort(
				(a, b) => Number(a.shared) - Number(b.shared) || a.name.localeCompare(b.name, 'sv')
			);
			showSave = false;
			viewName = '';
			viewShared = false;
		} catch (err) {
			viewError = err instanceof ApiError ? (err.problem?.detail ?? err.message) : String(err);
		}
	}
	async function removeView(v: SavedView) {
		if (v.shared && !confirm(`Ta bort den delade vyn "${v.name}" för alla?`)) return;
		await api.deleteView(v.id);
		views = views.filter((x) => x.id !== v.id);
	}

	const limit = $derived(Number(effective.get('limit')));
	const offset = $derived(Number(params.get('offset') ?? 0));
	let showColumns = $state(false);

	const fmtDate = (d?: string | null) => (d ? new Date(d).toLocaleDateString('sv-SE') : '');
	const sexLabel = (s: string) => ({ female: 'Flicka/kvinna', male: 'Pojke/man', other: 'Annat', unknown: 'Okänt' })[s] ?? s;
	function attendanceClass(p: number) {
		return p >= 0.75 ? 'good' : p >= 0.5 ? 'ok' : 'low';
	}
	function rowKey(r: MemberRow) {
		return r.id;
	}
</script>

<div class="grid-wrap card">
	<div class="toolbar">
		<input class="input search" type="search" placeholder="Sök namn eller medlemsnummer…" bind:value={q} aria-label="Sök" />

		{#if preset === 'members'}
			<div class="seg" role="group" aria-label="Visa">
				{#each [['', 'Alla'], ['false', 'Scouter'], ['true', 'Ledare']] as [v, label] (v)}
					<button class="chip" class:on={(params.get('leaders') ?? '') === v} onclick={() => setParams({ leaders: v })}>{label}</button>
				{/each}
			</div>
		{/if}

		<label class="inline">
			Född
			<input class="input year" type="number" placeholder="från" value={params.get('born_from') ?? ''} onchange={(e) => setParams({ born_from: e.currentTarget.value })} aria-label="Född från år" />
			–
			<input class="input year" type="number" placeholder="till" value={params.get('born_to') ?? ''} onchange={(e) => setParams({ born_to: e.currentTarget.value })} aria-label="Född till år" />
		</label>

		{#if preset === 'members' && troops.length > 0}
			<select class="input" value={params.get('troop_id') ?? ''} onchange={(e) => setParams({ troop_id: e.currentTarget.value })} aria-label="Avdelning">
				<option value="">Alla avdelningar</option>
				{#each troops as t (t.id)}
					<option value={t.id}>{t.name}</option>
				{/each}
			</select>
		{/if}

		<div class="spacer"></div>
		<div class="popover-anchor">
			<button class="btn" onclick={() => (showColumns = !showColumns)} aria-expanded={showColumns}>Kolumner ▾</button>
			{#if showColumns}
				<div class="popover card" role="dialog" aria-label="Välj kolumner">
					{#each columns as c (c.key)}
						<label><input type="checkbox" bind:checked={visible[c.key]} /> {c.label}</label>
					{/each}
				</div>
			{/if}
		</div>
		<div class="popover-anchor">
			<button class="btn" onclick={() => (showSave = !showSave)} aria-expanded={showSave}>☆ Spara vy</button>
			{#if showSave}
				<form class="popover card save-view" onsubmit={saveView} aria-label="Spara vy">
					<label>Namn <input class="input" bind:value={viewName} required maxlength="80" placeholder="t.ex. Ledare utan registerutdrag" /></label>
					<label class="check"><input type="checkbox" bind:checked={viewShared} /> Dela med alla som ser listan här</label>
					{#if viewError}<p class="error small">{viewError}</p>{/if}
					<div class="row">
						<button type="button" class="btn" onclick={() => (showSave = false)}>Avbryt</button>
						<button class="btn primary">Spara</button>
					</div>
				</form>
			{/if}
		</div>
	</div>

	<div class="chips">
		{#if preset === 'members'}
			{#each troopTypes.filter((t) => t.key !== 'annan') as t (t.key)}
				<button class="chip" class:on={selectedTypes.includes(t.key)} onclick={() => toggleType(t.key)}>{t.label}</button>
			{/each}
		{/if}
		<button class="chip" class:on={params.get('missing_guardian_email') === 'true'} onclick={() => toggleChip('missing_guardian_email', 'true')}>
			⚠ Vårdnadshavare saknar e-post
		</button>
		{#if preset === 'members' && canPolice}
			<button class="chip" class:on={params.get('police_check') === 'missing,expired,expiring'} onclick={() => toggleChip('police_check', 'missing,expired,expiring')}>
				⚠ Registerutdrag saknas/går ut
			</button>
		{/if}
		{#if preset === 'members' && canFee}
			<button class="chip" class:on={params.get('fee_status') === 'unpaid,overdue'} onclick={() => toggleChip('fee_status', 'unpaid,overdue')}>
				💳 Obetald avgift {data?.term ?? ''}
			</button>
		{/if}
		{#if filtersActive}
			<button class="btn ghost small" onclick={clearFilters}>✕ Rensa filter</button>
		{/if}
	</div>

	{#if views.length > 0}
		<div class="chips views">
			<span class="muted">Sparade vyer:</span>
			{#each views as v (v.id)}
				<span class="chip" class:on={activeQuery === normalize(v.query)} title={v.shared ? `Delad av ${v.owner_name}` : 'Bara för dig'}>
					{#if v.shared}<span aria-label="Delad">👥</span>{/if}
					<a href={`?${v.query}`}>{v.name}</a>
					{#if v.mine}<button class="x" onclick={() => removeView(v)} aria-label={`Ta bort vyn ${v.name}`}>×</button>{/if}
				</span>
			{/each}
		</div>
	{/if}

	{#if data && data.redacted_fields.length > 0}
		<p class="notice">
			🔒 Vissa uppgifter är dolda eftersom din roll inte ger behörighet:
			{[...new Set(data.redacted_fields.map((f) => redactedLabels[f] ?? f))].join(', ')}.
		</p>
	{/if}
	{#if error}
		<p class="notice error">Kunde inte hämta: {error}</p>
	{/if}

	{#if canManage && picked.size > 0}
		<div class="bulkbar" role="toolbar" aria-label="Massåtgärder">
			<strong>{picked.size} valda</strong>
			{#if preset === 'waiting'}
				<button class="btn primary small" onclick={() => (bulkAction = 'admit')} disabled={troops.length === 0}>Ta in i avdelning…</button>
			{:else}
				<button class="btn primary small" onclick={() => (bulkAction = 'move')} disabled={troops.length === 0}>Flytta till avdelning…</button>
			{/if}
			<button class="btn ghost small" onclick={() => picked.clear()}>Avmarkera</button>
		</div>
	{/if}

	<div class="table-scroll" class:loading>
		<table>
			<thead>
				<tr>
					{#if canManage}
						<th class="pick">
							<input
								type="checkbox"
								checked={allOnPage}
								{@attach (el: HTMLInputElement) => {
									el.indeterminate = someOnPage && !allOnPage;
								}}
								onchange={togglePage}
								aria-label="Markera alla på sidan"
							/>
						</th>
					{/if}
					{#each shown as c (c.key)}
						{@const s = sortState(c)}
						<th class:sortable={!!c.sort} class:right={c.align === 'right'} aria-sort={s ? (s.dir === '▲' ? 'ascending' : 'descending') : undefined}>
							{#if c.sort}
								<button class="th-btn" onclick={(e) => sortBy(c, e)} title="Klicka för att sortera, shift-klicka för flera nivåer">
									{c.label}
									{#if s}<span class="sort">{s.dir}{#if sortKeys.length > 1}<sub>{s.n}</sub>{/if}</span>{/if}
								</button>
							{:else}
								{c.label}
							{/if}
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each data?.items ?? [] as r, i (rowKey(r))}
					<tr class:selected={selected === r.id} class:picked={picked.has(r.id)} onclick={() => (selected = r.id)}>
						{#if canManage}
							<td class="pick" onclick={(e) => e.stopPropagation()}>
								<input
									type="checkbox"
									checked={picked.has(r.id)}
									onclick={(e) => togglePick(i, e)}
									aria-label={`Markera ${r.first_name} ${r.last_name}`}
								/>
							</td>
						{/if}
						{#each shown as c (c.key)}
							<td class:right={c.align === 'right'}>
								{#if c.key === 'name'}
									<button class="link" onclick={() => (selected = r.id)}>{r.last_name}, {r.first_name}</button>
								{:else if c.key === 'member_no'}
									<span class="mono">{r.member_no}</span>
								{:else if c.key === 'age'}
									{r.age}
								{:else if c.key === 'birth_date'}
									{fmtDate(r.birth_date)}
								{:else if c.key === 'troop'}
									{#if r.troop}<span class="badge blue">{r.troop.name}</span>{/if}
								{:else if c.key === 'patrol'}
									{r.patrol?.name.replace('Patrull ', '') ?? ''}
								{:else if c.key === 'role'}
									{#if r.is_leader}<span class="badge accent">Ledare</span>{:else if r.age < 26}<span class="muted">Scout</span>{/if}
								{:else if c.key === 'person_status'}
									{#if r.person_status !== 'active'}
										{@const s = personStatusLabels[r.person_status]}
										<span class="badge {s?.tone}">{s?.label ?? r.person_status}</span>
									{:else}
										<span class="muted">Aktiv</span>
									{/if}
								{:else if c.key === 'waiting_since'}
									{fmtDate(r.waiting_since)}
								{:else if c.key === 'wished'}
									{troopTypeLabel(r.wished_troop_type)}
								{:else if c.key === 'email'}
									{#if r.email}<a href={`mailto:${r.email}`} onclick={(e) => e.stopPropagation()}>{r.email}</a>{:else if data?.redacted_fields.includes('email')}<span class="muted">🔒</span>{/if}
								{:else if c.key === 'alt_email'}
									{#if r.alt_email}<a href={`mailto:${r.alt_email}`} onclick={(e) => e.stopPropagation()}>{r.alt_email}</a>{/if}
								{:else if c.key === 'mobile_phone'}
									<span class="nowrap">{r.mobile_phone ?? ''}</span>
								{:else if c.key === 'home_phone'}
									<span class="nowrap">{r.home_phone ?? ''}</span>
								{:else if c.key === 'national_id'}
									<span class="mono">{r.national_id_masked ?? '🔒'}</span>
								{:else if c.key === 'police_check'}
									{#if r.police_check && r.police_check !== 'not_required'}
										{@const s = policeCheckLabels[r.police_check]}
										<span class="badge {s.tone}" title={r.police_check_valid_until ? `Giltigt till ${fmtDate(r.police_check_valid_until)}` : ''}>{s.label}</span>
									{:else if !r.police_check && data?.redacted_fields.includes('police_check')}
										<span class="muted">🔒</span>
									{/if}
								{:else if c.key === 'fee'}
									{#if r.fee_status}
										{@const s = feeStatusLabels[r.fee_status]}
										<span class="badge {s.tone}" title={r.fee_due_date ? `Förfallodag ${fmtDate(r.fee_due_date)}` : ''}>{s.label}</span>
									{:else if data?.redacted_fields.includes('fee_status')}
										<span class="muted">🔒</span>
									{/if}
								{:else if c.key === 'fee_group'}
									{r.fee_group ?? ''}
								{:else if c.key === 'guardians'}
									{#if r.guardian_count > 0}
										{r.guardian_count}
										{#if r.guardians_missing_email}<span class="badge red" title="Ingen vårdnadshavare har e-post">saknar e-post</span>{/if}
									{/if}
								{:else if c.key === 'attendance'}
									{#if r.attendance_pct != null}
										<span class="att {attendanceClass(r.attendance_pct)}" title={`${Math.round(r.attendance_pct * 100)} % ${data?.term}`}>
											<span class="bar" style:width={`${Math.round(r.attendance_pct * 100)}%`}></span>
											<span class="pct">{Math.round(r.attendance_pct * 100)}%</span>
										</span>
									{/if}
								{:else if c.key === 'member_since'}
									{fmtDate(r.member_since)}
								{/if}
							</td>
						{/each}
					</tr>
				{:else}
					{#if !loading}
						<tr><td colspan={shown.length + (canManage ? 1 : 0)} class="empty">Inga träffar. Prova att ändra filtren.</td></tr>
					{/if}
				{/each}
			</tbody>
		</table>
	</div>

	<div class="footer">
		{#if data}
			<span>
				Visar {data.total === 0 ? 0 : offset + 1}–{Math.min(offset + limit, data.total)} av <strong>{data.total}</strong>
				<span class="muted"> · server {data.took_ms.toFixed(1)} ms</span>
			</span>
		{/if}
		<div class="spacer"></div>
		<label class="inline">
			Per sida
			<select class="input" value={String(limit)} onchange={(e) => setParams({ limit: e.currentTarget.value })}>
				{#each [25, 50, 100, 200, 500] as n (n)}<option value={String(n)}>{n}</option>{/each}
			</select>
		</label>
		<button class="btn" disabled={offset === 0} onclick={() => setParams({ offset: String(Math.max(0, offset - limit)) })}>‹ Föregående</button>
		<button class="btn" disabled={!data || offset + limit >= data.total} onclick={() => setParams({ offset: String(offset + limit) })}>Nästa ›</button>
	</div>
</div>

{#if selected}
	<PersonPanel id={selected} onclose={() => (selected = null)} onopen={(pid) => (selected = pid)} />
{/if}

{#if bulkAction}
	<BulkDialog
		groupId={bodyId}
		action={bulkAction}
		personIds={[...picked]}
		{troops}
		onclose={() => (bulkAction = null)}
		ondone={bulkDone}
	/>
{/if}

<svelte:window onkeydown={(e) => e.key === 'Escape' && ((selected = null), (showColumns = false), (bulkAction = null))} />

<style>
	.grid-wrap {
		padding: 0;
		overflow: hidden;
	}
	.toolbar,
	.chips,
	.footer {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4);
	}
	.chips {
		padding-top: 0;
	}
	.views a {
		color: inherit;
	}
	.save-view {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		width: 300px;
	}
	.save-view label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.88rem;
	}
	.save-view label.check {
		flex-direction: row;
		align-items: center;
		gap: 6px;
	}
	.save-view .row {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
	.save-view .error {
		color: var(--sn-red);
		margin: 0;
	}
	.x {
		border: none;
		background: none;
		cursor: pointer;
		padding: 0 2px;
		color: var(--muted);
	}
	.search {
		width: 280px;
	}
	.year {
		width: 84px;
	}
	.inline {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--muted);
		font-size: 0.9rem;
	}
	.seg {
		display: inline-flex;
		gap: 4px;
	}
	.spacer {
		flex: 1;
	}
	.small {
		font-size: 0.85rem;
		padding: 4px 8px;
	}
	.popover-anchor {
		position: relative;
	}
	.popover {
		position: absolute;
		right: 0;
		top: calc(100% + 6px);
		z-index: 20;
		display: grid;
		gap: 6px;
		min-width: 200px;
	}
	.notice {
		margin: 0 var(--space-4) var(--space-3);
		padding: var(--space-2) var(--space-3);
		background: var(--sn-accent-50);
		border-radius: var(--radius-sm);
		font-size: 0.88rem;
	}
	.notice.error {
		background: var(--sn-red-50);
		color: var(--sn-red);
	}
	.table-scroll {
		overflow: auto;
		max-height: calc(100vh - 330px);
		border-top: 1px solid var(--border);
		border-bottom: 1px solid var(--border);
		transition: opacity 0.15s;
	}
	.table-scroll.loading {
		opacity: 0.55;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.92rem;
	}
	th {
		position: sticky;
		top: 0;
		background: var(--surface);
		text-align: left;
		font-weight: 600;
		color: var(--muted);
		padding: 8px 12px;
		border-bottom: 1px solid var(--border);
		white-space: nowrap;
		z-index: 1;
	}
	.th-btn {
		border: none;
		background: none;
		padding: 0;
		font: inherit;
		color: inherit;
		cursor: pointer;
	}
	th.sortable:hover .th-btn {
		color: var(--primary);
	}
	.sort {
		color: var(--primary);
		margin-left: 2px;
	}
	td {
		padding: 7px 12px;
		border-bottom: 1px solid var(--border);
		white-space: nowrap;
	}
	.right {
		text-align: right;
	}
	.nowrap {
		white-space: nowrap;
	}
	tbody tr {
		cursor: pointer;
	}
	tbody tr:hover {
		background: var(--sn-blue-50);
	}
	tr.selected {
		background: var(--sn-blue-100) !important;
	}
	tr.picked {
		background: var(--sn-accent-50);
	}
	th.pick,
	td.pick {
		width: 36px;
		padding-right: 0;
		cursor: default;
	}
	.pick input {
		width: 16px;
		height: 16px;
		cursor: pointer;
	}
	.bulkbar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin: 0 var(--space-4) var(--space-3);
		padding: var(--space-2) var(--space-3);
		background: var(--sn-blue-50);
		border: 1px solid var(--sn-blue-100);
		border-radius: var(--radius-sm);
		font-size: 0.9rem;
	}
	.link {
		border: none;
		background: none;
		padding: 0;
		font: inherit;
		color: var(--primary);
		font-weight: 600;
		cursor: pointer;
	}
	.empty {
		text-align: center;
		padding: var(--space-6);
		color: var(--muted);
	}
	.att {
		position: relative;
		display: inline-block;
		width: 90px;
		height: 18px;
		border-radius: 9px;
		background: var(--sn-gray-100);
		overflow: hidden;
		vertical-align: middle;
	}
	.att .bar {
		position: absolute;
		inset: 0 auto 0 0;
	}
	.att.good .bar {
		background: #a5d6a7;
	}
	.att.ok .bar {
		background: #ffe082;
	}
	.att.low .bar {
		background: #ef9a9a;
	}
	.att .pct {
		position: relative;
		font-size: 0.75rem;
		font-weight: 600;
		padding-left: 8px;
		line-height: 18px;
		color: var(--sn-gray-900);
	}
</style>
