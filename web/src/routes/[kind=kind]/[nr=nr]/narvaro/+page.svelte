<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		api,
		attendanceLabels,
		fmtShort,
		fmtWhen,
		shiftTerm,
		termKey,
		type AttendanceMatrix,
		type AttendanceStatus,
		type AttendanceSummary,
		type Meeting
	} from '#lib/api/client.ts';
	import { bodyHref } from '#lib/bodies.ts';
	import MeetingPlanner from '#lib/components/MeetingPlanner.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);
	const isTroop = $derived(body.kind === 'troop');
	const termParam = $derived(page.url.searchParams.get('termin') ?? undefined);

	let matrix = $state<AttendanceMatrix | null>(null);
	let summary = $state<AttendanceSummary | null>(null);
	let error = $state<string | null>(null);
	let planning = $state(false);
	let reload = $state(0);

	$effect(() => {
		const key = `${body.kind}-${body.number}`;
		const term = termParam;
		void reload;
		let current = true;
		error = null;
		const req = isTroop ? api.attendance(key, term).then((m) => current && (matrix = m)) : api.attendanceSummary(key, term).then((s) => current && (summary = s));
		req.catch((e) => current && (error = e.message));
		return () => (current = false);
	});

	const term = $derived(matrix?.term ?? summary?.term ?? '');
	function setTerm(label: string) {
		const u = new URL(page.url.href);
		u.searchParams.set('termin', termKey(label));
		goto(u, { replace: true, reset: false });
	}

	const meetingHref = (m: { id: string }) => `${bodyHref(body, 'narvaro')}/${m.id}`;
	const past = $derived(matrix?.meetings.filter((m) => m.is_past) ?? []);
	const last = $derived(past.at(-1));
	const next = $derived(matrix?.meetings.find((m) => !m.is_past));
	const missing = $derived(past.filter((m) => m.present + m.absent + m.excused === 0));
	const avg = $derived.by(() => {
		const rec = past.reduce((s, m) => s + m.present + m.absent + m.excused, 0);
		return rec ? Math.round((past.reduce((s, m) => s + m.present, 0) / rec) * 100) : null;
	});
	const scouts = $derived(matrix?.people.filter((p) => !p.is_leader) ?? []);
	const leaders = $derived(matrix?.people.filter((p) => p.is_leader) ?? []);
	const pct = (p: { present: number; recorded: number }) => (p.recorded ? Math.round((p.present / p.recorded) * 100) : null);
	const cell = (s: string | null | undefined) => (s ? attendanceLabels[s as AttendanceStatus] : null);
	const kindIcon = (m: Meeting) => (m.kind === 'hike' ? '🥾' : m.kind === 'camp' ? '⛺' : m.kind === 'activity' ? '★' : '');

	function created(ms: Meeting[]) {
		planning = false;
		const t = ms[0] && `${ms[0].date >= `${ms[0].date.slice(0, 4)}-07-01` ? 'HT' : 'VT'}${ms[0].date.slice(0, 4)}`;
		if (t && t !== termKey(term)) setTerm(t);
		else reload++;
	}

	// Kalenderprenumeration: en ny adress skapas vid varje klick och ersätter den förra.
	let calendarUrl = $state<string | null>(null);
	async function subscribe() {
		if (calendarUrl) {
			calendarUrl = null;
			return;
		}
		calendarUrl = (await api.createCalendar(`${body.kind}-${body.number}`)).url;
	}
	async function revokeCalendar() {
		await api.deleteCalendar(`${body.kind}-${body.number}`);
		calendarUrl = null;
	}

	const totals = $derived.by(() => {
		const t = { planned: 0, held: 0, missing: 0, members: 0 };
		for (const r of summary?.troops ?? []) {
			t.planned += r.planned;
			t.held += r.held;
			t.missing += r.missing_attendance;
			t.members += r.members;
		}
		return t;
	});
</script>

<header class="head">
	<div>
		<h1>Närvaro <span class="muted">– {body.name}</span></h1>
		{#if term}
			<div class="term" role="group" aria-label="Termin">
				<button class="btn ghost small" onclick={() => setTerm(shiftTerm(term, -1))} aria-label="Föregående termin">‹</button>
				<strong>{term}</strong>
				<button class="btn ghost small" onclick={() => setTerm(shiftTerm(term, 1))} aria-label="Nästa termin">›</button>
			</div>
		{/if}
	</div>
	{#if matrix?.can_write}
		<button class="btn primary" onclick={() => (planning = true)}>＋ Planera möten</button>
	{/if}
	<button class="btn" onclick={subscribe}>📅 Kalender</button>
</header>

{#if calendarUrl}
	<div class="card calendar" role="region" aria-label="Kalenderprenumeration">
		<p>
			Prenumerera på {body.name}s möten i din kalender (Google, Outlook, iPhone). Adressen är personlig och ger
			bara mötestider, inga personuppgifter.
		</p>
		<div class="cal-row">
			<input class="input" readonly value={calendarUrl} aria-label="Kalenderadress" onfocus={(e) => e.currentTarget.select()} />
			<button class="btn" onclick={() => navigator.clipboard.writeText(calendarUrl!)}>Kopiera</button>
			<a class="btn" href={calendarUrl.replace(/^https?:/, 'webcal:')}>Öppna i kalendern</a>
			<button class="btn ghost" onclick={revokeCalendar}>Återkalla</button>
		</div>
	</div>
{/if}

{#if error}
	<p class="notice error">Kunde inte hämta: {error}</p>
{:else if isTroop && matrix}
	<section class="cards">
		<div class="card">
			<span class="muted small">Senaste mötet</span>
			{#if last}
				<strong>{last.name}</strong>
				<span class="muted">{fmtWhen(last.starts_at)}</span>
				{#if last.present + last.absent + last.excused === 0}
					<a class="btn primary" href={meetingHref(last)}>Registrera närvaro</a>
				{:else}
					<span>{last.present} närvarande{last.excused ? `, ${last.excused} förhinder` : ''}</span>
					<a class="btn" href={meetingHref(last)}>Öppna</a>
				{/if}
			{:else}
				<span class="muted">Inga genomförda möten</span>
			{/if}
		</div>
		<div class="card">
			<span class="muted small">Nästa möte</span>
			{#if next}
				<strong>{next.name}</strong>
				<span class="muted">{fmtWhen(next.starts_at)}{next.location ? ` · ${next.location}` : ''}</span>
				<a class="btn" href={meetingHref(next)}>Öppna</a>
			{:else}
				<span class="muted">Inga planerade möten</span>
				{#if matrix.can_write}<button class="btn" onclick={() => (planning = true)}>Planera</button>{/if}
			{/if}
		</div>
		<div class="card">
			<span class="muted small">Terminen</span>
			<strong>{past.length} av {matrix.meetings.length} möten genomförda</strong>
			<span class="muted">{avg !== null ? `Snittnärvaro ${avg} %` : 'Ingen närvaro registrerad'}</span>
			{#if missing.length > 0}<span class="badge red">{missing.length} {missing.length === 1 ? 'möte saknar' : 'möten saknar'} närvaro</span>{/if}
		</div>
	</section>

	{#if matrix.meetings.length === 0}
		<p class="muted">Inga möten {term}.{#if matrix.can_write} Planera terminens möten för att kunna registrera närvaro.{/if}</p>
	{:else}
		<div class="card matrix-wrap">
			<div class="table-scroll">
				<table class="matrix">
					<thead>
						<tr>
							<th class="name">Namn</th>
							{#each matrix.meetings as m (m.id)}
								<th class="m" class:future={!m.is_past} class:missing={m.is_past && m.present + m.absent + m.excused === 0}>
									<a href={meetingHref(m)} title={`${m.name}, ${fmtWhen(m.starts_at)}`}>{kindIcon(m)}{fmtShort(m.date)}</a>
								</th>
							{/each}
							<th class="right">Närvaro</th>
						</tr>
					</thead>
					{#each [{ label: 'Scouter', rows: scouts }, { label: 'Ledare', rows: leaders }] as group (group.label)}
						{#if group.rows.length > 0}
							<tbody>
								<tr class="group"><th colspan={matrix.meetings.length + 2}>{group.label} ({group.rows.length})</th></tr>
								{#each group.rows as p (p.id)}
									<tr>
										<th class="name" scope="row">
											{p.first_name} {p.last_name}
											{#if p.patrol}<span class="muted small"> · {p.patrol.replace('Patrull ', '')}</span>{/if}
										</th>
										{#each p.statuses as s, i (matrix.meetings[i].id)}
											{@const c = cell(s)}
											<td class="m {s ?? ''}" class:future={!matrix.meetings[i].is_past} title={c?.label ?? 'Ej registrerad'}>{c?.icon ?? ''}</td>
										{/each}
										<td class="right">
											{#if pct(p) !== null}<strong>{pct(p)} %</strong> <span class="muted small">{p.present}/{p.recorded}</span>{/if}
										</td>
									</tr>
								{/each}
							</tbody>
						{/if}
					{/each}
					<tfoot>
						<tr>
							<th class="name">Närvarande</th>
							{#each matrix.meetings as m (m.id)}<td class="m">{m.present || ''}</td>{/each}
							<td></td>
						</tr>
					</tfoot>
				</table>
			</div>
			<p class="legend muted small">
				✓ närvarande · ✗ frånvarande · F anmält förhinder · 🥾 hajk · ⛺ läger. Klicka på ett datum för att registrera eller ändra.
			</p>
		</div>
	{/if}
{:else if summary}
	{#if summary.troops.length === 0}
		<p class="muted">Inga avdelningar.</p>
	{:else}
		<div class="card table-card">
			<div class="table-scroll">
				<table>
					<thead>
						<tr>
							<th>Avdelning</th>
							<th class="right">Medlemmar</th>
							<th class="right">Möten</th>
							<th class="right">Närvaro</th>
							<th>Att göra</th>
							<th>Senaste mötet</th>
							<th>Nästa möte</th>
						</tr>
					</thead>
					<tbody>
						{#each summary.troops as t (t.troop.id)}
							<tr>
								<td>
									<a href={bodyHref(t.troop, 'narvaro') + (termParam ? `?termin=${termParam}` : '')}>{t.troop.name}</a>
									<span class="muted small">{t.troop_type_label ?? ''}</span>
								</td>
								<td class="right">{t.members}</td>
								<td class="right">{t.held} / {t.planned}</td>
								<td class="right">{t.attendance_pct != null ? `${Math.round(t.attendance_pct * 100)} %` : ''}</td>
								<td>
									{#if t.missing_attendance > 0}
										<span class="badge red">{t.missing_attendance} saknar närvaro</span>
									{:else if t.planned === 0 && t.troop_type !== 'annan'}
										<span class="badge accent">Inga möten planerade</span>
									{/if}
								</td>
								<td class="muted">
									{#if t.last_meeting && t.last_meeting_id}
										<a href={`${bodyHref(t.troop, 'narvaro')}/${t.last_meeting_id}`}>{fmtWhen(t.last_meeting)}</a>
									{/if}
								</td>
								<td class="muted">{t.next_meeting ? fmtWhen(t.next_meeting) : ''}</td>
							</tr>
						{/each}
					</tbody>
					<tfoot>
						<tr>
							<td>Summa</td>
							<td class="right">{totals.members}</td>
							<td class="right">{totals.held} / {totals.planned}</td>
							<td></td>
							<td>{#if totals.missing > 0}<span class="badge red">{totals.missing} saknar närvaro</span>{/if}</td>
							<td colspan="2"></td>
						</tr>
					</tfoot>
				</table>
			</div>
		</div>
	{/if}
{:else}
	<p class="muted">Laddar…</p>
{/if}

{#if planning}
	<MeetingPlanner {body} term={term || 'HT 2026'} onclose={() => (planning = false)} oncreated={created} />
{/if}

<style>
	.head {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--space-3);
		flex-wrap: wrap;
	}
	h1 .muted {
		font-weight: 400;
	}
	.calendar {
		margin-top: var(--space-3);
	}
	.calendar p {
		margin: 0 0 var(--space-2);
	}
	.cal-row {
		display: flex;
		gap: var(--space-2);
		flex-wrap: wrap;
	}
	.cal-row .input {
		flex: 1 1 320px;
		font-family: var(--mono);
		font-size: 0.82rem;
	}
	.term {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-top: -6px;
	}
	.btn.small {
		padding: 2px 10px;
	}
	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
		gap: var(--space-3);
		margin: var(--space-4) 0;
	}
	.cards .card {
		display: flex;
		flex-direction: column;
		gap: 6px;
		align-items: flex-start;
	}
	.small {
		font-size: 0.82rem;
	}
	.matrix-wrap,
	.table-card {
		padding: 0;
		overflow: hidden;
	}
	.table-scroll {
		overflow-x: auto;
	}
	table {
		border-collapse: collapse;
		width: 100%;
		font-size: 0.9rem;
	}
	th,
	td {
		padding: 6px 8px;
		border-bottom: 1px solid var(--border);
		white-space: nowrap;
		text-align: left;
	}
	thead th {
		background: var(--sn-gray-100);
		color: var(--muted);
		font-weight: 600;
	}
	.right {
		text-align: right;
	}
	.matrix th.name {
		position: sticky;
		left: 0;
		background: var(--surface);
		z-index: 1;
		font-weight: 500;
		min-width: 190px;
	}
	.matrix thead th.name {
		background: var(--sn-gray-100);
	}
	.matrix .m {
		text-align: center;
		padding: 6px 4px;
		min-width: 38px;
	}
	.matrix thead .m a {
		font-size: 0.78rem;
		color: var(--primary);
		text-decoration: none;
	}
	.matrix thead .m.missing a {
		color: var(--sn-red);
		font-weight: 700;
	}
	.matrix .future {
		opacity: 0.55;
	}
	td.present {
		color: var(--badge-green-text);
		background: var(--sn-green-50);
		font-weight: 700;
	}
	td.absent {
		color: var(--badge-red-text);
	}
	td.excused {
		color: var(--badge-accent-text);
		background: var(--sn-accent-50);
		font-weight: 700;
	}
	tr.group th {
		background: var(--sn-gray-100);
		color: var(--muted);
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		position: sticky;
		left: 0;
	}
	tfoot td,
	tfoot th {
		font-weight: 600;
		border-top: 2px solid var(--border);
	}
	.legend {
		padding: var(--space-2) var(--space-4) var(--space-3);
		margin: 0;
	}
	a {
		color: var(--primary);
	}
</style>
