<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, type Persona } from '#lib/api/client.ts';
	import { session } from '#lib/session.svelte.ts';

	let personas = $state<Persona[]>([]);
	let error = $state<string | null>(null);
	let busy = $state<string | null>(null);
	let config = $state<{ dev_login: boolean; scoutid: boolean } | null>(null);

	$effect(() => {
		api.authConfig()
			.then((c) => {
				config = c;
				if (c.dev_login) return api.personas().then((p) => (personas = p));
			})
			.catch(() => (error = 'Kunde inte nå servern.'));
	});

	const returnTo = $derived(page.url.searchParams.get('return_to') ?? '/');
	const scoutidHref = $derived(`/auth/login?return_to=${encodeURIComponent(returnTo)}`);
	const loginErrors: Record<string, string> = {
		avbruten: 'Inloggningen avbröts i ScoutID.',
		ogiltig: 'Inloggningen hade gått ut eller startades i en annan flik. Försök igen.',
		okand: 'Ditt ScoutID är inte kopplat till någon medlem i Scoutnet 2. Kontakta kårens medlemsregistrerare.',
		teknik: 'Något gick fel i kontakten med ScoutID. Försök igen om en stund.',
		scoutid_nere: 'ScoutID svarar inte just nu. Försök igen om en stund.'
	};
	const loginError = $derived(loginErrors[page.url.searchParams.get('fel') ?? ''] ?? null);

	async function login(p: Persona) {
		busy = p.account_id;
		try {
			await api.devLogin(p.account_id);
			await session.load();
			goto(returnTo);
		} finally {
			busy = null;
		}
	}

	const icon = (persona: string) =>
		persona.startsWith('Förbund')
			? '🏛️'
			: persona.startsWith('Distrikt')
				? '🗺️'
				: persona.startsWith('Kårordf')
					? '🏕️'
					: persona.startsWith('Medlemsreg')
						? '🗂️'
						: persona.startsWith('Avdelning')
							? '🧭'
							: persona.startsWith('Ledare')
								? '🔥'
								: persona.startsWith('Scout')
									? '🎒'
									: '👪';
</script>

<main class="login">
	<section class="hero">
		<img class="hero-logo" src="/brand/scouterna_tagline_white_RGB.svg" alt="Scouterna" />
		<h1>Scoutnet <span>2</span></h1>
		<p>Prototyp av ett nytt medlemssystem för Scouterna. Snabbt, säkert och byggt API-först.</p>
	</section>

	<section class="card panel">
		{#if loginError}<p class="error" role="alert">{loginError}</p>{/if}
		{#if config?.scoutid}
			<a class="btn primary scoutid" href={scoutidHref}>Logga in med ScoutID</a>
			<p class="muted small">
				Lokal demo av ScoutID: logga in med ett medlemsnummer från <code>deploy/keycloak/README.md</code> och lösenordet
				<code>scout</code>. I produktion sker all inloggning via nya ScoutID med MFA.
			</p>
		{:else}
			<button class="btn primary scoutid" disabled title="ScoutID är inte konfigurerat eller svarar inte">Logga in med ScoutID</button>
			<p class="muted small">
				{config ? 'ScoutID svarar inte (starta den lokala Keycloaken med docker compose up -d).' : ''}
				I produktion sker all inloggning via nya ScoutID med MFA.
			</p>
		{/if}

		{#if config?.dev_login}
			<h2>Demo – välj roll</h2>
		{/if}
		{#if error}
			<p class="error">{error}</p>
		{/if}
		<ul class="personas">
			{#each personas as p (p.account_id)}
				<li>
					<button class="persona" onclick={() => login(p)} disabled={busy !== null}>
						<span class="icon" aria-hidden="true">{icon(p.persona)}</span>
						<span class="text">
							<strong>{p.persona}</strong>
							<span class="muted">{p.name}</span>
						</span>
						{#if busy === p.account_id}<span class="muted">…</span>{/if}
					</button>
				</li>
			{/each}
		</ul>
		<p class="muted small">All data är syntetisk. Personnummer är Skatteverkets testpersonnummer (980–999).</p>
	</section>
</main>

<style>
	.login {
		min-height: 100vh;
		display: grid;
		grid-template-columns: 1fr 1fr;
		align-items: center;
		gap: var(--space-6);
		padding: var(--space-6);
		background: linear-gradient(135deg, var(--sn-blue-900), var(--sn-blue-600));
	}
	.hero {
		color: #fff;
		max-width: 520px;
		justify-self: end;
	}
	.hero-logo {
		width: min(360px, 80%);
		height: auto;
		margin-bottom: var(--space-5);
	}
	.hero h1 {
		font-size: 3rem;
	}
	.hero h1 span {
		color: var(--sn-accent);
	}
	.hero p {
		font-size: 1.2rem;
		color: #dbe8f6;
	}
	.panel {
		max-width: 520px;
		width: 100%;
	}
	.scoutid {
		width: 100%;
		display: flex;
		justify-content: center;
		box-sizing: border-box;
		text-decoration: none;
		padding: 12px;
		font-size: 1.05rem;
	}
	code {
		font-size: 0.85em;
	}
	.small {
		font-size: 0.85rem;
	}
	.personas {
		list-style: none;
		padding: 0;
		margin: 0;
		display: grid;
		gap: var(--space-2);
	}
	.persona {
		width: 100%;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		text-align: left;
		padding: 10px 12px;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--surface);
		cursor: pointer;
	}
	.persona:hover {
		border-color: var(--primary);
		background: var(--sn-blue-50);
	}
	.icon {
		font-size: 1.5rem;
	}
	.text {
		display: flex;
		flex-direction: column;
	}
	.error {
		color: var(--sn-red);
	}
	@media (max-width: 900px) {
		.login {
			grid-template-columns: 1fr;
			padding: var(--space-4);
		}
		.hero {
			justify-self: start;
		}
		.hero h1 {
			font-size: 2.2rem;
		}
	}
</style>
