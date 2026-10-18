<script lang="ts">
	import { api, ApiError, type Statistics } from '#lib/api/client.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);

	let stats = $state<Statistics | null>(null);
	let error = $state<string | null>(null);

	$effect(() => {
		const key = `${body.kind}-${body.number}`;
		let current = true;
		stats = null;
		error = null;
		api.statistics(key)
			.then((s) => current && (stats = s))
			.catch((e) => current && (error = e instanceof ApiError ? (e.problem?.detail ?? e.message) : String(e)));
		return () => (current = false);
	});

	const pct = (n: number, max: number) => (n <= 0 || max <= 0 ? '0' : `${Math.max(1, Math.round((n / max) * 100))}%`);
	const ageMax = $derived(Math.max(1, ...(stats?.age_groups.flatMap((a) => [a.female, a.male]) ?? [])));
	const trendMax = $derived(Math.max(1, ...(stats?.trend.map((t) => t.active) ?? [])));
	const flowMax = $derived(Math.max(1, ...(stats?.flows.flatMap((f) => [f.joined, f.left]) ?? [])));
	const change = $derived.by(() => {
		const t = stats?.trend;
		if (!t || t.length < 2 || t[0].active === 0) return null;
		return Math.round(((t[t.length - 1].active - t[0].active) / t[0].active) * 100);
	});
	const months = (days?: number | null) => (days == null ? '–' : days < 60 ? `${Math.round(days)} dagar` : `${Math.round(days / 30.4)} mån`);
	const nf = new Intl.NumberFormat('sv-SE');
	const dec = new Intl.NumberFormat('sv-SE', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
</script>

<header class="head">
	<h1>Statistik <span class="muted">– {body.name}</span></h1>
	{#if stats}<p class="muted">Termin {stats.term} · {nf.format(stats.totals.active)} aktiva medlemmar · beräknad på {stats.took_ms.toFixed(0)} ms</p>{/if}
</header>

{#if error}
	<p class="card error">{error}</p>
{:else if !stats}
	<p class="muted">Räknar…</p>
{:else}
	{@const s = stats}
	<section class="totals">
		<div class="card stat"><span class="num">{nf.format(s.totals.scouts)}</span><span class="muted">scouter</span></div>
		<div class="card stat"><span class="num">{nf.format(s.totals.leaders)}</span><span class="muted">ledare</span></div>
		<div class="card stat">
			<span class="num">{s.totals.leaders > 0 ? dec.format(s.totals.scouts / s.totals.leaders) : '–'}</span>
			<span class="muted">scouter per ledare</span>
		</div>
		{#if s.waiting.length > 0 || s.totals.waiting > 0}
			<div class="card stat"><span class="num">{nf.format(s.totals.waiting)}</span><span class="muted">i väntelistan</span></div>
		{/if}
		{#if s.totals.troops > 0}
			<div class="card stat"><span class="num">{nf.format(s.totals.troops)}</span><span class="muted">avdelningar</span></div>
		{/if}
		{#if change != null}
			<div class="card stat">
				<span class="num" class:up={change > 0} class:down={change < 0}>{change > 0 ? '+' : ''}{change} %</span>
				<span class="muted">sedan {s.trend[0].label}</span>
			</div>
		{/if}
	</section>

	<div class="cols">
		<section class="card">
			<h2>Medlemsutveckling</h2>
			<p class="muted small">Aktiva medlemmar vid varje terminsstart och i dag.</p>
			<div class="columns" role="img" aria-label="Aktiva medlemmar per termin">
				{#each s.trend as t (t.date)}
					<div class="colwrap">
						<span class="val">{nf.format(t.active)}</span>
						<div class="col" class:today={t.label === 'I dag'} style:height={pct(t.active, trendMax)}></div>
						<span class="lbl">{t.label}</span>
					</div>
				{/each}
			</div>
			{#if s.flows.length > 0}
				<h3>Nya och slutade per termin</h3>
				<div class="columns flows" role="img" aria-label="Nya och slutade medlemmar per termin">
					{#each s.flows as f (f.term)}
						<div class="colwrap">
							<div class="pair">
								<div class="col joined" style:height={pct(f.joined, flowMax)} title={`${f.joined} nya`}></div>
								<div class="col left" style:height={pct(f.left, flowMax)} title={`${f.left} slutade`}></div>
							</div>
							<span class="lbl">{f.term}</span>
							<span class="lbl small"><span class="j">+{f.joined}</span> <span class="l">−{f.left}</span></span>
						</div>
					{/each}
				</div>
				<p class="legend small"><span class="dot joined"></span> nya <span class="dot left"></span> slutade</p>
			{/if}
		</section>

		<section class="card">
			<h2>Ålder och kön</h2>
			<p class="muted small">Aktiva medlemmar, scouter och ledare.</p>
			<div class="pyramid" role="img" aria-label="Åldersfördelning per kön">
				<span class="ph">Flickor och kvinnor</span><span></span><span class="ph">Pojkar och män</span>
				{#each s.age_groups as a (a.label)}
					<div class="side f">
						<span class="n">{a.female || ''}</span>
						<div class="bar f" style:width={pct(a.female, ageMax)}></div>
					</div>
					<span class="age">{a.label}</span>
					<div class="side m">
						<div class="bar m" style:width={pct(a.male, ageMax)}></div>
						<span class="n">{a.male || ''}{#if a.other > 0} <span class="muted">(+{a.other} annat)</span>{/if}</span>
					</div>
				{/each}
			</div>
		</section>
	</div>

	{#if s.troop_types.length > 0}
		<section class="card">
			<h2>Per gren</h2>
			<table>
				<thead>
					<tr>
						<th>Gren</th><th class="r">Avdelningar</th><th class="r">Scouter</th><th class="r">Ledare</th>
						<th class="r">Scouter per ledare</th>
						{#if s.attendance_included}<th>Närvaro {s.term}</th>{/if}
					</tr>
				</thead>
				<tbody>
					{#each s.troop_types as t (t.troop_type)}
						<tr>
							<td>{t.label}</td>
							<td class="r">{t.troops}</td>
							<td class="r">{nf.format(t.scouts)}</td>
							<td class="r">{nf.format(t.leaders)}</td>
							<td class="r">
								{#if t.scouts_per_leader != null}
									<span class:warn={t.scouts_per_leader > 8}>{dec.format(t.scouts_per_leader)}</span>
								{:else}–{/if}
							</td>
							{#if s.attendance_included}
								<td>
									{#if t.attendance_pct != null}
										<span class="att"><span class="fill" style:width={`${Math.round(t.attendance_pct * 100)}%`}></span><span class="pct">{Math.round(t.attendance_pct * 100)} %</span></span>
									{:else}<span class="muted">–</span>{/if}
								</td>
							{/if}
						</tr>
					{/each}
				</tbody>
			</table>
			{#if !s.attendance_included}
				<p class="muted small">Närvaro visas när modulen Närvaro är på.</p>
			{/if}
		</section>
	{/if}

	{#if s.waiting.length > 0}
		<section class="card">
			<h2>Väntelistan</h2>
			<table>
				<thead><tr><th>Önskad gren</th><th class="r">Väntar</th><th class="r">Median väntetid</th></tr></thead>
				<tbody>
					{#each s.waiting as w (w.label)}
						<tr><td>{w.label}</td><td class="r">{w.count}</td><td class="r">{months(w.median_days)}</td></tr>
					{/each}
				</tbody>
			</table>
		</section>
	{/if}

	<p class="muted small">Statistiken visar bara antal och innehåller inga personuppgifter.</p>
{/if}

<style>
	.head h1 .muted {
		font-weight: 400;
	}
	.head p {
		margin-top: 0;
	}
	.totals {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}
	.stat {
		display: flex;
		flex-direction: column;
	}
	.num {
		font-size: 1.8rem;
		font-weight: 700;
		color: var(--primary-strong);
	}
	.num.up {
		color: var(--sn-green);
	}
	.num.down {
		color: var(--sn-red);
	}
	.cols {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
		gap: var(--space-4);
		margin-bottom: var(--space-4);
	}
	section.card {
		margin-bottom: var(--space-4);
	}
	.cols section.card {
		margin-bottom: 0;
	}
	h2 {
		margin: 0 0 4px;
		font-size: 1.15rem;
	}
	h3 {
		font-size: 0.95rem;
		margin: var(--space-4) 0 var(--space-2);
	}
	.small {
		font-size: 0.82rem;
	}
	.columns {
		display: flex;
		align-items: flex-end;
		gap: var(--space-2);
		height: 190px;
		padding-top: var(--space-2);
	}
	.columns.flows {
		height: 150px;
	}
	.colwrap {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: flex-end;
		height: 100%;
		min-width: 0;
	}
	.col {
		width: 70%;
		background: var(--sn-blue-500);
		border-radius: 4px 4px 0 0;
		min-height: 2px;
	}
	.col.today {
		background: var(--sn-accent);
	}
	.pair {
		display: flex;
		align-items: flex-end;
		gap: 3px;
		width: 80%;
		height: 100%;
	}
	.pair .col {
		width: 50%;
	}
	.col.joined,
	.dot.joined {
		background: var(--sn-green);
	}
	.col.left,
	.dot.left {
		background: var(--sn-red);
	}
	.val {
		font-size: 0.78rem;
		font-weight: 600;
	}
	.lbl {
		font-size: 0.75rem;
		color: var(--muted);
		white-space: nowrap;
		margin-top: 4px;
	}
	.lbl .j {
		color: var(--sn-green);
	}
	.lbl .l {
		color: var(--sn-red);
	}
	.legend {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--muted);
	}
	.dot {
		display: inline-block;
		width: 10px;
		height: 10px;
		border-radius: 2px;
	}
	.pyramid {
		display: grid;
		grid-template-columns: 1fr auto 1fr;
		gap: 4px var(--space-2);
		align-items: center;
		margin-top: var(--space-2);
	}
	.ph {
		font-size: 0.78rem;
		color: var(--muted);
		text-align: center;
	}
	.side {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 18px;
	}
	.side.f {
		justify-content: flex-end;
	}
	.bar {
		height: 100%;
		border-radius: 3px;
	}
	.bar.f {
		background: #c2185b;
		opacity: 0.75;
	}
	.bar.m {
		background: var(--sn-blue-500);
		opacity: 0.85;
	}
	.n {
		font-size: 0.78rem;
		white-space: nowrap;
	}
	.age {
		font-size: 0.8rem;
		font-weight: 600;
		text-align: center;
		min-width: 44px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.92rem;
		margin-top: var(--space-2);
	}
	th {
		text-align: left;
		color: var(--muted);
		font-weight: 600;
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
	}
	td {
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
	}
	.r {
		text-align: right;
	}
	.warn {
		color: var(--sn-red);
		font-weight: 600;
	}
	.att {
		position: relative;
		display: inline-block;
		width: 120px;
		height: 18px;
		border-radius: 9px;
		background: var(--sn-gray-100);
		overflow: hidden;
		vertical-align: middle;
	}
	.att .fill {
		position: absolute;
		inset: 0 auto 0 0;
		background: #a5d6a7;
	}
	.att .pct {
		position: relative;
		font-size: 0.75rem;
		font-weight: 600;
		padding-left: 8px;
		line-height: 18px;
		color: var(--sn-gray-900);
	}
	.error {
		color: var(--sn-red);
	}
</style>
