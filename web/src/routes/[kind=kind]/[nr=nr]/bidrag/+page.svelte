<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, ApiError, fmtSekExact as fmtSek, fmtShort, shiftTerm, termKey, type GrantReport } from '#lib/api/client.ts';
	import { bodyHref } from '#lib/bodies.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);
	const isTroop = $derived(body.kind === 'troop');
	const termParam = $derived(page.url.searchParams.get('termin') ?? undefined);

	let report = $state<GrantReport | null>(null);
	let error = $state<string | null>(null);
	let showAll = $state(false);
	let dakError = $state<string | null>(null);
	let dakBusy = $state<number | null>(null);

	$effect(() => {
		const key = `${body.kind}-${body.number}`;
		const term = termParam;
		let current = true;
		error = null;
		api.grants(key, term)
			.then((r) => current && (report = r))
			.catch((e) => current && (error = e instanceof ApiError ? (e.problem?.detail ?? e.message) : e.message));
		return () => (current = false);
	});

	function setTerm(label: string) {
		const u = new URL(page.url.href);
		u.searchParams.set('termin', termKey(label));
		goto(u, { replace: true, reset: false });
	}

	const shown = $derived(report ? (isTroop || showAll ? report.meetings : report.meetings.filter((m) => !m.approved)) : []);

	// DAK-filen hämtas med fetch så att felet "kräver ny inloggning" kan visas i stället för en JSON-sida.
	async function downloadDak(troop: { number: number; name: string }) {
		dakError = null;
		dakBusy = troop.number;
		try {
			const res = await fetch(`/v1/bodies/troop-${troop.number}/grants/dak${report ? `?term=${termKey(report.term)}` : ''}`, {
				credentials: 'same-origin'
			});
			if (!res.ok) {
				const problem = res.headers.get('content-type')?.includes('json') ? await res.json() : null;
				dakError =
					problem?.code === 'step_up_required'
						? 'step_up'
						: res.status === 403
							? 'DAK-filen innehåller personnummer och kräver behörighet att visa personnummer (t.ex. medlemsregistrerare).'
							: (problem?.detail ?? problem?.title ?? `Fel ${res.status}`);
				return;
			}
			const blob = await res.blob();
			const a = document.createElement('a');
			a.href = URL.createObjectURL(blob);
			a.download = `DAK-${troop.name}-${report ? termKey(report.term) : ''}.xml`;
			a.click();
			setTimeout(() => URL.revokeObjectURL(a.href), 1000);
		} finally {
			dakBusy = null;
		}
	}
	const reloginHref = $derived(`/login?return_to=${encodeURIComponent(page.url.pathname + page.url.search)}`);
	const kr = (ore: number) => (ore ? fmtSek(ore) : '–');
</script>

<header class="head">
	<div>
		<h1>Bidrag <span class="muted">– {body.name}</span></h1>
		{#if report}
			<div class="term" role="group" aria-label="Termin">
				<button class="btn ghost small" onclick={() => setTerm(shiftTerm(report!.term, -1))} aria-label="Föregående termin">‹</button>
				<strong>{report.term}</strong>
				<button class="btn ghost small" onclick={() => setTerm(shiftTerm(report!.term, 1))} aria-label="Nästa termin">›</button>
			</div>
		{/if}
	</div>
	{#if isTroop && report?.can_export_dak && report.held > 0}
		<button class="btn primary" onclick={() => downloadDak(body)} disabled={dakBusy !== null}>⬇ Ladda ner DAK-fil</button>
	{/if}
</header>

{#if dakError}
	<p class="notice error" role="alert">
		{#if dakError === 'step_up'}
			DAK-filen innehåller personnummer och kräver att du har loggat in nyligen. <a href={reloginHref}>Logga in igen</a>.
		{:else}
			{dakError}
		{/if}
	</p>
{/if}

{#if error}
	<p class="notice error">{error}</p>
{:else if report}
	<section class="card rules">
		{#if report.rules}
			{@const r = report.rules}
			<div>
				<h2>{r.name}</h2>
				<p class="muted">
					Deltagare {r.min_age}–{r.max_age} år · minst {r.min_participants} deltagare och {r.min_leaders} ledare (minst {r.min_leader_age} år) ·
					minst {r.min_minutes} minuter · {fmtSek(r.amount_per_female_ore)} per flicka och {fmtSek(r.amount_per_male_ore)} per pojke och sammankomst.
					Ålder räknas som det år man fyller.
				</p>
			</div>
		{/if}
		{#each report.problems as p (p)}<p class="problem">⚠ {p}</p>{/each}
		<div class="totals">
			<div><span class="num">{report.approved}</span><span class="muted">av {report.held} sammankomster godkända</span></div>
			<div><span class="num">{kr(report.amount_ore)}</span><span class="muted">beräknat bidrag {report.term}</span></div>
		</div>
	</section>

	{#if !isTroop && report.troops.length > 0}
		<div class="card table-card">
			<table>
				<thead>
					<tr><th>Avdelning</th><th class="right">Genomförda</th><th class="right">Godkända</th><th class="right">Bidrag</th><th></th></tr>
				</thead>
				<tbody>
					{#each report.troops as t (t.troop.id)}
						<tr>
							<td><a href={bodyHref(t.troop, 'bidrag') + (termParam ? `?termin=${termParam}` : '')}>{t.troop.name}</a></td>
							<td class="right">{t.held}</td>
							<td class="right">
								{t.approved}
								{#if t.approved < t.held}<span class="badge red">{t.held - t.approved} underkända</span>{/if}
							</td>
							<td class="right">{kr(t.amount_ore)}</td>
							<td class="right">
								{#if report.can_export_dak}
									<button class="btn small" onclick={() => downloadDak(t.troop)} disabled={dakBusy !== null}>⬇ DAK</button>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
				<tfoot>
					<tr><td>Summa</td><td class="right">{report.held}</td><td class="right">{report.approved}</td><td class="right">{kr(report.amount_ore)}</td><td></td></tr>
				</tfoot>
			</table>
		</div>
	{/if}

	<section>
		<div class="section-head">
			<h2>{isTroop || showAll ? 'Sammankomster' : 'Underkända sammankomster'}</h2>
			{#if !isTroop}
				<label class="inline"><input type="checkbox" bind:checked={showAll} /> Visa alla</label>
			{/if}
		</div>
		{#if shown.length === 0}
			<p class="muted">{report.held === 0 ? 'Inga genomförda sammankomster den här terminen.' : 'Alla sammankomster är godkända. 🎉'}</p>
		{:else}
			<div class="card table-card">
				<div class="table-scroll">
					<table>
						<thead>
							<tr>
								<th>Datum</th>
								{#if !isTroop}<th>Avdelning</th>{/if}
								<th>Sammankomst</th>
								<th class="right">Min</th>
								<th class="right">Deltagare</th>
								<th class="right">Ledare</th>
								<th>Status</th>
								<th class="right">Bidrag</th>
							</tr>
						</thead>
						<tbody>
							{#each shown as m (m.meeting_id)}
								<tr class:bad={!m.approved}>
									<td class="nowrap">{fmtShort(m.date)} {m.time}</td>
									{#if !isTroop}<td>{m.troop.name}</td>{/if}
									<td>
										<a href={`${bodyHref(m.troop, 'narvaro')}/${m.meeting_id}`}>{m.name}</a>
										{#if m.kind !== 'meeting'}<span class="badge blue">hajk</span>{/if}
									</td>
									<td class="right">{m.duration_min}</td>
									<td class="right" title={`${m.eligible_female} flickor, ${m.eligible_male} pojkar berättigade av ${m.participants} närvarande`}>
										{m.eligible}<span class="muted small">/{m.participants}</span>
									</td>
									<td class="right">{m.qualifying_leaders}</td>
									<td>
										{#if m.approved}<span class="badge green">✓ Godkänd</span>{:else}<span class="badge red">✗ Underkänd</span>{/if}
										{#each m.issues as i (i)}<div class="issue small">{i}</div>{/each}
									</td>
									<td class="right">{kr(m.amount_ore)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/if}
		<p class="muted small">
			Deltagare räknas om de är inom åldersgränserna och har giltigt personnummer (kommunen kräver det). Kön för bidraget följer
			personnumret. Underlaget räknas om direkt när närvaron ändras.
		</p>
	</section>
{:else}
	<p class="muted">Laddar…</p>
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
	.term {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-top: -6px;
	}
	.btn.small {
		padding: 2px 10px;
	}
	.rules {
		margin: var(--space-4) 0;
	}
	.rules h2 {
		margin: 0 0 4px;
	}
	.problem {
		color: var(--badge-red-text);
		font-weight: 600;
		margin: var(--space-2) 0;
	}
	.totals {
		display: flex;
		gap: var(--space-6);
		flex-wrap: wrap;
		margin-top: var(--space-3);
	}
	.totals div {
		display: flex;
		flex-direction: column;
	}
	.num {
		font-size: 1.8rem;
		font-weight: 700;
		color: var(--primary-strong);
	}
	.table-card {
		padding: 0;
		overflow: hidden;
		margin-bottom: var(--space-4);
	}
	.table-scroll {
		overflow-x: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}
	th,
	td {
		padding: 7px 10px;
		border-bottom: 1px solid var(--border);
		text-align: left;
		vertical-align: top;
	}
	thead th {
		background: var(--sn-gray-100);
		color: var(--muted);
	}
	tfoot td {
		font-weight: 600;
		border-top: 2px solid var(--border);
	}
	.right {
		text-align: right;
	}
	.nowrap {
		white-space: nowrap;
	}
	tr.bad td {
		background: color-mix(in srgb, var(--sn-red-50) 50%, transparent);
	}
	.issue {
		color: var(--badge-red-text);
		margin-top: 2px;
	}
	.small {
		font-size: 0.82rem;
	}
	.section-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.inline {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	a {
		color: var(--primary);
	}
</style>
