<script lang="ts">
	import { api, ApiError, type BodyChild, type BulkResult } from '#lib/api/client.ts';

	type Action = 'move' | 'admit';
	let {
		groupId,
		action,
		personIds,
		troops,
		onclose,
		ondone
	}: {
		groupId: string;
		action: Action;
		personIds: string[];
		troops: BodyChild[];
		onclose: () => void;
		ondone: (r: BulkResult) => void;
	} = $props();

	const title = $derived(action === 'move' ? 'Flytta till avdelning' : 'Ta in från väntelistan');
	const outcomeLabels: Record<string, { label: string; tone: string }> = {
		ok: { label: 'OK', tone: 'green' },
		warning: { label: 'Obs', tone: 'accent' },
		skip: { label: 'Hoppas över', tone: '' }
	};

	let troopId = $state('');
	let patrolId = $state('');
	let patrols = $state<BodyChild[]>([]);
	let preview = $state<BulkResult | null>(null);
	let result = $state<BulkResult | null>(null);
	let busy = $state(false);
	let error = $state<string | null>(null);

	const message = (e: unknown) => (e instanceof ApiError ? (e.problem?.detail ?? e.message) : String(e));

	$effect(() => {
		const id = troopId;
		patrolId = '';
		patrols = [];
		if (!id) return;
		let current = true;
		api.children(id)
			.then((c) => current && (patrols = c.filter((x) => x.kind === 'patrol')))
			.catch(() => {});
		return () => (current = false);
	});

	// Torrkörning så fort målet ändras: användaren ser exakt vad som händer innan något ändras.
	$effect(() => {
		const troop = troopId;
		const patrol = patrolId;
		preview = null;
		error = null;
		if (!troop) return;
		let current = true;
		api.bulk(groupId, { action, person_ids: personIds, troop_id: troop, patrol_id: patrol || null, dry_run: true })
			.then((r) => current && (preview = r))
			.catch((e) => current && (error = message(e)));
		return () => (current = false);
	});

	async function run() {
		busy = true;
		error = null;
		try {
			result = await api.bulk(groupId, {
				action,
				person_ids: personIds,
				troop_id: troopId,
				patrol_id: patrolId || null,
				dry_run: false
			});
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
	const shown = $derived(result ?? preview);
</script>

<div class="backdrop" onclick={() => (result ? ondone(result) : onclose())} aria-hidden="true"></div>
<div class="dialog card" role="dialog" aria-modal="true" aria-labelledby="bulk-title">
	<h2 id="bulk-title">{title} <span class="muted">({personIds.length} valda)</span></h2>

	{#if !result}
		<div class="grid">
			<label>
				Avdelning
				<select class="input" bind:value={troopId} required>
					<option value="" disabled>Välj avdelning…</option>
					{#each troops as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
				</select>
			</label>
			<label>
				Patrull
				<select class="input" bind:value={patrolId} disabled={patrols.length === 0}>
					<option value="">{patrols.length === 0 ? 'Avdelningen har inga patruller' : 'Ingen patrull'}</option>
					{#each patrols as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
				</select>
			</label>
		</div>
	{/if}

	{#if error}<p class="error">{error}</p>{/if}

	{#if shown}
		<p class="summary" class:done={!!result}>
			{#if result}
				✔ Klart. {result.changed} {result.changed === 1 ? 'person har' : 'personer har'}
				{action === 'move' ? 'flyttats' : 'tagits in'}{#if result.skipped > 0}, {result.skipped} hoppades över{/if}.
			{:else}
				Förhandsgranskning: {shown.changed} {shown.changed === 1 ? 'person' : 'personer'}
				{action === 'move' ? 'flyttas' : 'tas in'}{#if shown.skipped > 0}, {shown.skipped} hoppas över{/if}. Inget är ändrat ännu.
			{/if}
		</p>
		<div class="rows">
			<table>
				<thead><tr><th>Namn</th><th>Från</th><th></th><th>Till</th><th>Status</th></tr></thead>
				<tbody>
					{#each shown.rows as r (r.person_id)}
						{@const o = outcomeLabels[r.outcome] ?? { label: r.outcome, tone: '' }}
						<tr class:skip={r.outcome === 'skip'}>
							<td>{r.name}</td>
							<td class="muted">{r.from}</td>
							<td class="muted">→</td>
							<td>{r.to}</td>
							<td>
								<span class="badge {o.tone}">{o.label}</span>
								{#if r.message}<span class="msg">{r.message}</span>{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else if troopId && !error}
		<p class="muted">Förhandsgranskar…</p>
	{/if}

	<div class="actions">
		{#if result}
			<button class="btn primary" onclick={() => result && ondone(result)}>Stäng</button>
		{:else}
			<button type="button" class="btn" onclick={onclose}>Avbryt</button>
			<button class="btn primary" disabled={busy || !preview || preview.changed === 0} onclick={run}>
				{busy ? 'Genomför…' : `${action === 'move' ? 'Flytta' : 'Ta in'} ${preview?.changed ?? 0} ${preview?.changed === 1 ? 'person' : 'personer'}`}
			</button>
		{/if}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 35%);
		z-index: 40;
	}
	.dialog {
		position: fixed;
		z-index: 50;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: min(760px, calc(100vw - 24px));
		max-height: calc(100vh - 40px);
		display: flex;
		flex-direction: column;
	}
	h2 {
		margin-top: 0;
	}
	h2 .muted {
		font-weight: 400;
		font-size: 0.9em;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
		gap: var(--space-3);
		margin-bottom: var(--space-3);
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.88rem;
		color: var(--muted);
	}
	label .input {
		color: var(--text);
	}
	.summary {
		margin: 0 0 var(--space-2);
		padding: var(--space-2) var(--space-3);
		background: var(--sn-accent-50);
		border-radius: var(--radius-sm);
		font-size: 0.92rem;
	}
	.summary.done {
		background: var(--sn-green-50);
	}
	.rows {
		overflow-y: auto;
		min-height: 0;
		flex: 1;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.88rem;
	}
	th {
		position: sticky;
		top: 0;
		background: var(--surface);
		text-align: left;
		color: var(--muted);
		font-weight: 600;
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
	}
	td {
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
		vertical-align: top;
	}
	tr.skip td {
		color: var(--muted);
	}
	.msg {
		display: block;
		font-size: 0.8rem;
		color: var(--muted);
		margin-top: 2px;
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}
	.error {
		color: var(--sn-red);
	}
</style>
