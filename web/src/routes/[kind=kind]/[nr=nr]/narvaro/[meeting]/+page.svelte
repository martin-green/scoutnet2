<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		api,
		ApiError,
		attendanceLabels,
		fmtWeekday,
		meetingKindLabels,
		type AttendanceStatus,
		type MeetingDetail,
		type RosterEntry
	} from '#lib/api/client.ts';
	import { bodyHref } from '#lib/bodies.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	// Skrivbar härledning: följer sidans data, men ersätts av serverns svar efter varje sparning.
	let detail = $derived<MeetingDetail>(data.detail);
	// Lokal närvaro per person. Ändringar sparas automatiskt efter en kort paus.
	let marks = $state<Record<string, AttendanceStatus | null>>({});
	let saveState = $state<'idle' | 'pending' | 'saving' | 'saved' | 'error'>('idle');
	let saveError = $state<string | null>(null);
	let editing = $state(false);

	$effect(() => {
		marks = Object.fromEntries(data.detail.roster.map((r) => [r.id, (r.status as AttendanceStatus | null) ?? null]));
		saveState = 'idle';
		editing = false;
	});

	const m = $derived(detail.meeting);
	const readOnly = $derived(!detail.can_write || m.locked);
	const scouts = $derived(detail.roster.filter((r) => !r.is_leader));
	const leaders = $derived(detail.roster.filter((r) => r.is_leader));
	const counts = $derived.by(() => {
		const c = { present: 0, absent: 0, excused: 0, none: 0 };
		for (const r of detail.roster) c[marks[r.id] ?? 'none']++;
		return c;
	});
	const base = $derived(bodyHref(data.body, 'narvaro'));

	let timer: ReturnType<typeof setTimeout> | undefined;
	let seq = 0;
	function schedule() {
		saveState = 'pending';
		clearTimeout(timer);
		timer = setTimeout(save, 600);
	}
	async function save() {
		const mine = ++seq;
		saveState = 'saving';
		const entries = Object.entries(marks)
			.filter((e): e is [string, AttendanceStatus] => e[1] !== null)
			.map(([person_id, status]) => ({ person_id, status }));
		try {
			const res = await api.saveAttendance(m.id, entries);
			if (mine === seq) {
				detail = res;
				saveState = 'saved';
			}
		} catch (e) {
			if (mine === seq) {
				saveState = 'error';
				saveError = e instanceof ApiError ? (e.problem?.detail ?? e.message) : String(e);
			}
		}
	}

	function set(r: RosterEntry, status: AttendanceStatus | null) {
		if (readOnly) return;
		marks[r.id] = status;
		schedule();
	}
	// Ett tryck på raden växlar mellan närvarande och frånvarande.
	const toggle = (r: RosterEntry) => set(r, marks[r.id] === 'present' ? 'absent' : 'present');
	const excuse = (r: RosterEntry) => set(r, marks[r.id] === 'excused' ? null : 'excused');
	function allPresent() {
		if (readOnly) return;
		for (const r of detail.roster) if (marks[r.id] !== 'excused') marks[r.id] = 'present';
		schedule();
	}
	function markRestAbsent() {
		if (readOnly) return;
		for (const r of detail.roster) if (!marks[r.id]) marks[r.id] = 'absent';
		schedule();
	}

	// Redigering av mötet.
	let form = $state({ name: '', date: '', time: '', duration_min: 90, location: '' });
	function startEdit() {
		form = { name: m.name, date: m.date, time: m.time, duration_min: m.duration_min, location: m.location ?? '' };
		editing = true;
	}
	async function saveEdit(e: SubmitEvent) {
		e.preventDefault();
		try {
			detail = await api.updateMeeting(m.id, { ...form, duration_min: Number(form.duration_min) });
			editing = false;
		} catch (err) {
			alert(err instanceof ApiError ? (err.problem?.detail ?? err.message) : String(err));
		}
	}
	async function remove() {
		if (!confirm(`Ta bort ${m.name} ${fmtWeekday(m.date)}? Registrerad närvaro tas också bort.`)) return;
		await api.deleteMeeting(m.id);
		goto(base);
	}

	const statusText = $derived(
		{ idle: '', pending: 'Ändrat…', saving: 'Sparar…', saved: 'Sparat ✓', error: `Kunde inte spara: ${saveError}` }[saveState]
	);
</script>

<svelte:window onbeforeunload={(e) => { if (saveState === 'pending' || saveState === 'saving') e.preventDefault(); }} />

<header class="head">
	<nav class="pager" aria-label="Bläddra mellan möten">
		{#if detail.previous_id}<a class="btn ghost" href={`${base}/${detail.previous_id}`} aria-label="Föregående möte">‹</a>{/if}
		<div>
			<h1>{m.name}</h1>
			<p class="muted">
				{fmtWeekday(m.date)} kl. {m.time} · {m.duration_min} min{m.location ? ` · ${m.location}` : ''}
				{#if m.kind !== 'meeting'}<span class="badge blue">{meetingKindLabels[m.kind]}</span>{/if}
				{#if m.locked}<span class="badge">🔒 Rapporterat</span>{/if}
			</p>
		</div>
		{#if detail.next_id}<a class="btn ghost" href={`${base}/${detail.next_id}`} aria-label="Nästa möte">›</a>{/if}
	</nav>
	{#if detail.can_write && !m.locked && !editing}
		<button class="btn ghost" onclick={startEdit}>Ändra mötet</button>
	{/if}
</header>

{#if editing}
	<form class="card edit" onsubmit={saveEdit}>
		<label>Namn <input class="input" bind:value={form.name} required maxlength="100" /></label>
		<label>Datum <input class="input" type="date" bind:value={form.date} required /></label>
		<label>Tid <input class="input" type="time" bind:value={form.time} required /></label>
		<label>Minuter <input class="input" type="number" min="5" bind:value={form.duration_min} required /></label>
		<label>Plats <input class="input" bind:value={form.location} /></label>
		<div class="edit-actions">
			<button type="button" class="btn danger" onclick={remove}>Ta bort mötet</button>
			<span class="spacer"></span>
			<button type="button" class="btn" onclick={() => (editing = false)}>Avbryt</button>
			<button class="btn primary">Spara</button>
		</div>
	</form>
{/if}

<div class="bar card">
	<div class="tally">
		<strong>{counts.present}</strong> av {detail.roster.length} närvarande
		{#if counts.excused}<span class="muted"> · {counts.excused} förhinder</span>{/if}
		{#if counts.absent}<span class="muted"> · {counts.absent} frånvarande</span>{/if}
		{#if counts.none && !readOnly}<span class="muted"> · {counts.none} ej markerade</span>{/if}
	</div>
	<span class="save {saveState}" role="status" aria-live="polite">{statusText}</span>
	{#if !readOnly}
		<div class="bulk">
			<button class="btn" onclick={allPresent}>Alla närvarande</button>
			{#if counts.none > 0}<button class="btn" onclick={markRestAbsent}>Övriga frånvarande</button>{/if}
		</div>
	{/if}
</div>

{#if readOnly}
	<p class="notice">
		{m.locked ? '🔒 Mötet är rapporterat och kan inte ändras.' : '👁 Du kan se närvaron men inte registrera den.'}
	</p>
{/if}

{#each [{ label: 'Scouter', rows: scouts }, { label: 'Ledare', rows: leaders }] as group (group.label)}
	{#if group.rows.length > 0}
		<h2 class="group">{group.label} <span class="muted">({group.rows.length})</span></h2>
		<ul class="roster">
			{#each group.rows as r (r.id)}
				{@const s = marks[r.id]}
				<li class={s ?? 'none'}>
					<button class="person" onclick={() => toggle(r)} disabled={readOnly} aria-pressed={s === 'present'}>
						<span class="mark" aria-hidden="true">{s ? attendanceLabels[s].icon : ''}</span>
						<span class="who">
							<strong>{r.first_name} {r.last_name}</strong>
							<span class="muted small">
								{r.age} år{r.patrol ? ` · ${r.patrol}` : ''}{!r.placed ? ' · inte placerad här' : ''}
							</span>
						</span>
						<span class="sr-only">{s ? attendanceLabels[s].label : 'Ej registrerad'}</span>
					</button>
					<button class="excuse" class:on={s === 'excused'} onclick={() => excuse(r)} disabled={readOnly} aria-pressed={s === 'excused'} title="Anmält förhinder">
						Förhinder
					</button>
				</li>
			{/each}
		</ul>
	{/if}
{/each}

{#if detail.roster.length === 0}
	<p class="muted">Ingen är placerad i avdelningen på mötesdagen.</p>
{/if}

<style>
	.head {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	.pager {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.pager h1 {
		margin: 0;
	}
	.pager p {
		margin: 2px 0 0;
	}
	.pager .btn {
		font-size: 1.4rem;
		padding: 4px 12px;
	}
	.edit {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
		gap: var(--space-3);
		margin: var(--space-3) 0;
	}
	.edit label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.85rem;
		color: var(--muted);
	}
	.edit-actions {
		grid-column: 1 / -1;
		display: flex;
		gap: var(--space-2);
	}
	.spacer {
		flex: 1;
	}
	.btn.danger {
		color: var(--sn-red);
	}
	.bar {
		position: sticky;
		top: 0;
		z-index: 2;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
		margin: var(--space-3) 0;
		padding: var(--space-3) var(--space-4);
	}
	.tally {
		font-size: 1.05rem;
	}
	.save {
		font-size: 0.88rem;
		color: var(--muted);
	}
	.save.saved {
		color: var(--badge-green-text);
	}
	.save.error {
		color: var(--sn-red);
	}
	.bulk {
		display: flex;
		gap: var(--space-2);
		margin-left: auto;
	}
	h2.group {
		font-size: 0.85rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--muted);
		margin: var(--space-4) 0 var(--space-2);
	}
	.roster {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
		gap: var(--space-2);
	}
	.roster li {
		display: flex;
		align-items: stretch;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--surface);
		overflow: hidden;
	}
	.roster li.present {
		border-color: var(--sn-green);
		background: var(--sn-green-50);
	}
	.roster li.excused {
		border-color: var(--sn-accent);
		background: var(--sn-accent-50);
	}
	.roster li.absent {
		opacity: 0.75;
	}
	.person {
		flex: 1;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		min-height: 56px;
		padding: var(--space-2) var(--space-3);
		border: 0;
		background: none;
		font: inherit;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}
	.person:disabled {
		cursor: default;
	}
	.mark {
		flex: none;
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		border: 2px solid var(--border);
		font-weight: 800;
	}
	.present .mark {
		background: var(--sn-green);
		border-color: var(--sn-green);
		color: #fff;
	}
	.absent .mark {
		color: var(--badge-red-text);
		border-color: var(--badge-red-text);
	}
	.excused .mark {
		background: var(--sn-accent);
		border-color: var(--sn-accent);
		color: var(--sn-blue-900);
	}
	.who {
		display: flex;
		flex-direction: column;
	}
	.small {
		font-size: 0.82rem;
	}
	.excuse {
		border: 0;
		border-left: 1px solid var(--border);
		background: none;
		color: var(--muted);
		font: inherit;
		font-size: 0.8rem;
		padding: 0 var(--space-3);
		cursor: pointer;
	}
	.excuse.on {
		color: var(--badge-accent-text);
		font-weight: 700;
	}
	.excuse:disabled {
		cursor: default;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0 0 0 0);
	}
	@media (max-width: 600px) {
		.roster {
			grid-template-columns: 1fr;
		}
		.bulk {
			margin-left: 0;
			width: 100%;
		}
		.bulk .btn {
			flex: 1;
		}
	}
</style>
