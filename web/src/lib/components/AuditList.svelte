<script lang="ts">
	import { api, ApiError, fmtDateTime, type AuditLog } from '#lib/api/client.ts';

	let { personId, self = false }: { personId: string; self?: boolean } = $props();

	let log = $state<AuditLog | null>(null);
	let error = $state<string | null>(null);
	let showAll = $state(false);

	$effect(() => {
		const id = personId;
		let current = true;
		log = null;
		error = null;
		api.personAudit(id)
			.then((l) => current && (log = l))
			.catch((e) => current && (error = e instanceof ApiError && e.status === 404 ? 'Du saknar behörighet att se loggen.' : String(e)));
		return () => (current = false);
	});

	const others = $derived(log?.entries.filter((e) => !e.by_self) ?? []);
	const entries = $derived(showAll ? (log?.entries ?? []) : others);
	const icon = (action: string) =>
		action === 'person.read'
			? '👁'
			: action.startsWith('person.national_id')
				? '🔑'
				: action.startsWith('bulk.')
					? '↪'
					: action.startsWith('grant.')
						? '📄'
						: action.startsWith('compat.')
							? '🔌'
							: '•';
</script>

{#if error}
	<p class="muted">{error}</p>
{:else if !log}
	<p class="muted">Laddar…</p>
{:else}
	<p class="lead">
		{#if log.views_by_others === 0}
			{self ? 'Ingen annan' : 'Ingen'} har öppnat {self ? 'dina' : 'personens'} uppgifter det senaste året.
		{:else}
			{self ? 'Dina' : 'Personens'} uppgifter har öppnats <strong>{log.views_by_others}</strong>
			{log.views_by_others === 1 ? 'gång' : 'gånger'} av {self ? 'andra' : 'andra än personen själv'} det senaste året.
		{/if}
	</p>
	{#if entries.length > 0}
		<ul class="log">
			{#each entries as e, i (i)}
				<li>
					<span class="icon" aria-hidden="true">{icon(e.action)}</span>
					<div>
						<div>{e.description}</div>
						<div class="muted small">
							{fmtDateTime(e.at)} ·
							{#if e.by_self}
								{self ? 'du själv' : 'personen själv'}
							{:else if e.actor}
								{e.actor}{e.actor_roles.length > 0 ? ` (${e.actor_roles.join('; ')})` : ''}
							{:else}
								{e.action.startsWith('compat.') ? 'via API-nyckel' : 'systemet'}
							{/if}
						</div>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
	{#if log.entries.length > others.length}
		<label class="toggle">
			<input type="checkbox" bind:checked={showAll} />
			Visa även {self ? 'mina egna' : 'personens egna'} visningar ({log.entries.length - others.length})
		</label>
	{/if}
	<p class="muted small">
		Varje visning av personkortet, visat personnummer, flytt och export registreras. Loggen kan inte ändras i efterhand.
	</p>
{/if}

<style>
	.lead {
		margin-top: 0;
	}
	.log {
		list-style: none;
		margin: 0 0 var(--space-3);
		padding: 0;
		max-height: 420px;
		overflow-y: auto;
		border-top: 1px solid var(--border);
	}
	.log li {
		display: flex;
		gap: var(--space-2);
		padding: 8px 0;
		border-bottom: 1px solid var(--border);
		font-size: 0.9rem;
	}
	.icon {
		width: 1.4em;
		text-align: center;
		flex: none;
	}
	.small {
		font-size: 0.8rem;
	}
	.toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 0.88rem;
		margin-bottom: var(--space-2);
	}
</style>
