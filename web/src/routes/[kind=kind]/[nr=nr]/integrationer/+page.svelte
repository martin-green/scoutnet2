<script lang="ts">
	import { api, ApiError, fmtDate, fmtDateTime, type ApiKeyInfo, type CreatedApiKey } from '#lib/api/client.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);
	const key = $derived(`${body.kind}-${body.number}`);
	const allowed = $derived(body.kind === 'group' && body.permissions.includes('members_manage'));
	const canContact = $derived(body.permissions.includes('members_contact_read'));
	const canSsno = $derived(body.permissions.includes('national_id_read'));

	const scopeLabels: Record<string, string> = {
		memberlist: 'Medlemslista',
		'memberlist:contact': 'Kontaktuppgifter',
		'memberlist:ssno': 'Personnummer'
	};

	let keys = $state<ApiKeyInfo[]>([]);
	let error = $state<string | null>(null);
	let name = $state('');
	let contact = $state(false);
	let ssno = $state(false);
	let busy = $state(false);
	let created = $state<CreatedApiKey | null>(null);
	let copied = $state(false);

	const message = (e: unknown) => (e instanceof ApiError ? (e.problem?.detail ?? e.message) : String(e));

	$effect(() => {
		if (!allowed) return;
		const k = key;
		let current = true;
		api.apiKeys(k)
			.then((list) => current && (keys = list))
			.catch((e) => current && (error = message(e)));
		return () => (current = false);
	});

	async function create(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = null;
		try {
			const scopes = [...(contact ? ['memberlist:contact'] : []), ...(ssno ? ['memberlist:ssno'] : [])];
			created = await api.createApiKey(key, { name: name.trim(), scopes });
			keys = [...keys, created.info];
			name = '';
			contact = false;
			ssno = false;
			copied = false;
		} catch (err) {
			error = err instanceof ApiError && err.status === 403 ? 'Du kan inte ge en integration mer behörighet än du själv har.' : message(err);
		} finally {
			busy = false;
		}
	}

	async function revoke(k: ApiKeyInfo) {
		if (!confirm(`Återkalla nyckeln "${k.name}"? Integrationen slutar fungera direkt.`)) return;
		try {
			await api.revokeApiKey(k.id);
			keys = keys.filter((x) => x.id !== k.id);
			if (created?.info.id === k.id) created = null;
		} catch (err) {
			error = message(err);
		}
	}

	async function copy(text: string) {
		await navigator.clipboard.writeText(text);
		copied = true;
	}
	const curl = $derived(created ? `curl -u ${created.username}:${created.key} ${created.endpoint}` : '');
</script>

<header>
	<h1>Integrationer <span class="muted">– {body.name}</span></h1>
	<p class="muted lead">
		API-nycklar låter andra system, till exempel Skojjt, kårens webbplats eller en e-postlista, hämta kårens medlemslista i
		<strong>samma format som dagens Scoutnet</strong> (<code>/api/group/memberlist</code>). Befintliga integrationer behöver
		bara byta adress och nyckel.
	</p>
</header>

{#if !allowed}
	<p class="card muted">Integrationer hanteras per kår av den som får hantera medlemskap (t.ex. medlemsregistrerare).</p>
{:else}
	{#if created}
		<section class="card created" aria-live="polite">
			<h2>✔ Nyckeln "{created.info.name}" är skapad</h2>
			<p><strong>Kopiera nyckeln nu.</strong> Den lagras bara som ett fingeravtryck och kan inte visas igen.</p>
			<dl>
				<dt>Adress</dt>
				<dd class="mono">{created.endpoint}</dd>
				<dt>Användarnamn</dt>
				<dd class="mono">{created.username}</dd>
				<dt>Nyckel</dt>
				<dd class="secret">
					<span class="mono">{created.key}</span>
					<button class="btn small" onclick={() => created && copy(created.key)}>{copied ? 'Kopierad ✔' : 'Kopiera'}</button>
				</dd>
			</dl>
			<p class="muted small">Exempel (HTTP Basic):</p>
			<pre class="mono">{curl}</pre>
			<button class="btn" onclick={() => (created = null)}>Jag har sparat nyckeln</button>
		</section>
	{/if}

	<div class="grid">
		<section class="card">
			<h2>Aktiva nycklar</h2>
			{#if keys.length === 0}
				<p class="muted">Kåren har inga API-nycklar.</p>
			{:else}
				<table>
					<thead><tr><th>Namn</th><th>Får hämta</th><th>Skapad</th><th>Senast använd</th><th></th></tr></thead>
					<tbody>
						{#each keys as k (k.id)}
							<tr>
								<td><strong>{k.name}</strong></td>
								<td>
									{#each k.scopes as s (s)}
										<span class="badge" class:red={s === 'memberlist:ssno'} class:blue={s === 'memberlist:contact'}>{scopeLabels[s] ?? s}</span>
									{/each}
								</td>
								<td>{fmtDate(k.created_at)}{#if k.created_by}<div class="muted small">{k.created_by}</div>{/if}</td>
								<td>{k.last_used_at ? fmtDateTime(k.last_used_at) : 'Aldrig'}</td>
								<td><button class="btn ghost small" onclick={() => revoke(k)}>Återkalla</button></td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</section>

		<section class="card">
			<h2>Ny nyckel</h2>
			<form onsubmit={create}>
				<label class="field">
					Namn
					<input class="input" bind:value={name} required maxlength="80" placeholder="t.ex. Skojjt närvaro" />
				</label>
				<fieldset>
					<legend>Integrationen får hämta</legend>
					<label class="check"><input type="checkbox" checked disabled /> Namn, födelsedatum, avdelning, patrull och roller</label>
					<label class="check" class:off={!canContact}>
						<input type="checkbox" bind:checked={contact} disabled={!canContact} /> Adress, e-post, telefon och anhöriga
					</label>
					<label class="check" class:off={!canSsno}>
						<input type="checkbox" bind:checked={ssno} disabled={!canSsno} /> Personnummer
						{#if !canSsno}<span class="muted small">(kräver behörighet att se personnummer)</span>{/if}
					</label>
				</fieldset>
				{#if ssno}
					<p class="warn small">Ge bara personnummer till system som måste ha dem, t.ex. bidragsansökan till kommunen.</p>
				{/if}
				{#if error}<p class="error">{error}</p>{/if}
				<button class="btn primary" disabled={busy || !name.trim()}>{busy ? 'Skapar…' : 'Skapa nyckel'}</button>
			</form>
			<p class="muted small">
				🔒 Varje hämtning loggas. Medlemmarna ser den under <em>Vem har sett mina uppgifter</em>, med integrationens namn.
				Personer med skyddad identitet lämnas aldrig ut med adress eller personnummer.
			</p>
		</section>
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
		grid-template-columns: minmax(0, 3fr) minmax(280px, 2fr);
		gap: var(--space-4);
		align-items: start;
	}
	@media (max-width: 900px) {
		.grid {
			grid-template-columns: minmax(0, 1fr);
		}
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9rem;
	}
	th {
		text-align: left;
		color: var(--muted);
		font-weight: 600;
		padding: 6px 8px;
		border-bottom: 1px solid var(--border);
	}
	td {
		padding: 8px;
		border-bottom: 1px solid var(--border);
		vertical-align: top;
	}
	td .badge {
		margin: 0 4px 4px 0;
	}
	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		align-items: flex-start;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 100%;
		font-size: 0.88rem;
		color: var(--muted);
	}
	.field .input {
		color: var(--text);
	}
	fieldset {
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		padding: var(--space-2) var(--space-3);
		width: 100%;
		box-sizing: border-box;
	}
	legend {
		font-size: 0.88rem;
		color: var(--muted);
		padding: 0 4px;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 0.9rem;
		padding: 3px 0;
	}
	.check.off {
		color: var(--muted);
	}
	.created {
		border: 2px solid var(--sn-green);
		margin-bottom: var(--space-4);
	}
	.created h2 {
		margin-top: 0;
	}
	dl {
		display: grid;
		grid-template-columns: 120px 1fr;
		gap: 6px var(--space-3);
	}
	dt {
		color: var(--muted);
	}
	dd {
		margin: 0;
		word-break: break-all;
	}
	.secret {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-wrap: wrap;
	}
	.secret .mono {
		background: var(--sn-accent-50);
		padding: 2px 6px;
		border-radius: 4px;
	}
	pre {
		background: var(--sn-gray-100);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-sm);
		overflow-x: auto;
		font-size: 0.82rem;
	}
	.small {
		font-size: 0.82rem;
	}
	.warn {
		background: var(--sn-accent-50);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-sm);
		margin: 0;
	}
	.error {
		color: var(--sn-red);
		margin: 0;
	}
</style>
