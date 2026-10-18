<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { api, ApiError, type BodyModule } from '#lib/api/client.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);
	const key = $derived(`${body.kind}-${body.number}`);

	let modules = $state<BodyModule[]>([]);
	let error = $state<string | null>(null);
	let busy = $state<string | null>(null);

	$effect(() => {
		const k = key;
		let current = true;
		api.bodyModules(k)
			.then((m) => current && (modules = m))
			.catch((e) => current && (error = message(e)));
		return () => (current = false);
	});

	const message = (e: unknown) => (e instanceof ApiError ? (e.problem?.detail ?? e.message) : String(e));
	const available = $derived(modules.filter((m) => m.availability === 'available'));
	const planned = $derived(modules.filter((m) => m.availability === 'planned'));
	const canChangeAny = $derived(available.some((m) => m.can_change));

	async function change(m: BodyModule, body: { enabled: boolean | null; locked?: boolean }) {
		busy = m.id;
		error = null;
		try {
			modules = await api.setModule(key, m.id, body);
			// Menyn och sidorna följer enhetens moduler.
			await invalidateAll();
		} catch (e) {
			error = message(e);
		} finally {
			busy = null;
		}
	}

	function sourceText(m: BodyModule): string {
		if (m.source === 'own') return `Valt för ${body.name}`;
		if (m.source === 'inherited') return `Ärvt från ${m.source_body}`;
		return m.chosen ? 'På som standard' : 'Av som standard';
	}
</script>

<header>
	<h1>Moduler <span class="muted">– {body.name}</span></h1>
	<p class="muted lead">
		Scoutnet 2 består av en kärna (medlemmar, organisation och behörighet) och moduler som kåren väljer att använda.
		Ett val gäller {body.name} och allt under, tills en underliggande enhet gör ett eget val.
		{#if body.kind !== 'group'}Här kan du också låsa valet, så att kårerna inte kan ändra det.{/if}
		Data finns kvar när en modul stängs av.
	</p>
</header>

{#if error}<p class="card error" role="alert">{error}</p>{/if}
{#if modules.length > 0 && !canChangeAny}
	<p class="notice">Du kan se modulerna här, men inte ändra dem. Det gör kårens ordförande, distriktet eller förbundet.</p>
{/if}

<div class="grid">
	{#each available as m (m.id)}
		<article class="card module" class:on={m.enabled}>
			<div class="top">
				<span class="icon" aria-hidden="true">{m.icon}</span>
				<div class="title">
					<h2>{m.name}</h2>
					<span class="badge" class:green={m.enabled}>{m.enabled ? 'På' : 'Av'}</span>
				</div>
				{#if m.can_change}
					<button
						class="switch"
						class:checked={m.chosen}
						role="switch"
						aria-checked={m.chosen}
						aria-label={`${m.name} ${m.chosen ? 'på' : 'av'}`}
						disabled={busy !== null}
						onclick={() => change(m, { enabled: !m.chosen, locked: m.locked_here && body.kind !== 'group' })}
					>
						<span class="knob"></span>
					</button>
				{/if}
			</div>
			<p class="desc">{m.description}</p>
			<ul class="meta">
				<li>{sourceText(m)}</li>
				{#if m.locked_by}<li>🔒 Låst av {m.locked_by}</li>{/if}
				{#if m.locked_here}<li>🔒 Låst för underliggande enheter</li>{/if}
				{#if m.depends_on.length > 0}<li>Kräver {m.depends_on.join(' och ')}</li>{/if}
				{#if m.chosen && m.missing.length > 0}
					<li class="warn">Av eftersom {m.missing.join(' och ')} är av</li>
				{/if}
			</ul>
			{#if m.can_change && (m.source === 'own' || m.can_lock)}
				<div class="actions">
					{#if m.can_lock}
						<label class="check">
							<input
								type="checkbox"
								checked={m.locked_here}
								disabled={busy !== null}
								onchange={(e) => change(m, { enabled: m.chosen, locked: e.currentTarget.checked })}
							/>
							Lås för underliggande enheter
						</label>
					{/if}
					{#if m.source === 'own'}
						<button class="btn ghost small" disabled={busy !== null} onclick={() => change(m, { enabled: null })}>
							Ärv i stället
						</button>
					{/if}
				</div>
			{/if}
		</article>
	{/each}
</div>

{#if planned.length > 0}
	<h2 class="section">Planerade moduler</h2>
	<div class="grid">
		{#each planned as m (m.id)}
			<article class="card module planned">
				<div class="top">
					<span class="icon" aria-hidden="true">{m.icon}</span>
					<div class="title">
						<h2>{m.name}</h2>
						<span class="badge accent">Planeras</span>
					</div>
				</div>
				<p class="desc">{m.description}</p>
			</article>
		{/each}
	</div>
{/if}

<style>
	h1 .muted {
		font-weight: 400;
	}
	.lead {
		max-width: 760px;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: var(--space-3);
		margin-bottom: var(--space-5);
	}
	.module {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		border-left: 4px solid var(--border);
	}
	.module.on {
		border-left-color: var(--sn-green);
	}
	.module.planned {
		opacity: 0.75;
		border-left-style: dashed;
	}
	.top {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}
	.icon {
		font-size: 1.8rem;
	}
	.title {
		flex: 1;
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.title h2 {
		margin: 0;
		font-size: 1.15rem;
	}
	.desc {
		margin: 0;
		font-size: 0.92rem;
	}
	.meta {
		list-style: none;
		margin: 0;
		padding: 0;
		font-size: 0.85rem;
		color: var(--muted);
		display: grid;
		gap: 2px;
	}
	.meta .warn {
		color: var(--sn-red);
	}
	.actions {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		flex-wrap: wrap;
		border-top: 1px solid var(--border);
		padding-top: var(--space-2);
		margin-top: auto;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 0.88rem;
	}
	.small {
		font-size: 0.85rem;
		padding: 4px 8px;
	}
	.switch {
		position: relative;
		width: 46px;
		height: 26px;
		border-radius: 13px;
		border: none;
		background: var(--sn-gray-300);
		cursor: pointer;
		flex: none;
		transition: background 0.15s;
	}
	.switch.checked {
		background: var(--sn-green);
	}
	.switch:disabled {
		opacity: 0.6;
		cursor: wait;
	}
	.knob {
		position: absolute;
		top: 3px;
		left: 3px;
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: white;
		box-shadow: 0 1px 3px rgb(0 0 0 / 30%);
		transition: transform 0.15s;
	}
	.switch.checked .knob {
		transform: translateX(20px);
	}
	.section {
		font-size: 1.1rem;
	}
	.notice {
		padding: var(--space-2) var(--space-3);
		background: var(--sn-accent-50);
		border-radius: var(--radius-sm);
		font-size: 0.9rem;
	}
	.error {
		color: var(--sn-red);
	}
</style>
