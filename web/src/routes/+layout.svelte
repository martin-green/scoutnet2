<script lang="ts">
	import '#lib/styles/app.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { Body } from '#lib/api/client.ts';
	import { bodyHref } from '#lib/bodies.ts';
	import { session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const isLogin = $derived(page.url.pathname.startsWith('/login'));
	const contexts = $derived(session.me?.contexts ?? []);
	// Enheten man står i (från enhetssidornas layout), annars första arbetskontexten.
	const body = $derived(page.data.body as Body | undefined);
	const current = $derived(
		body ?? (contexts[0] ? { id: contexts[0].body_id, kind: contexts[0].body_kind, number: contexts[0].body_number } : null)
	);
	// Kontextväljaren visar den roll man arbetar under: enheten själv eller närmaste överliggande kontext.
	const contextId = $derived.by(() => {
		if (!current) return '';
		const ids = [current.id, ...(body?.ancestors.map((a) => a.id).reverse() ?? [])];
		return ids.find((id) => contexts.some((c) => c.body_id === id)) ?? '';
	});

	$effect(() => {
		if (isLogin) return;
		session.load().then((me) => {
			// Efter inloggning kommer man tillbaka till sidan man försökte öppna.
			if (!me) goto(`/login?return_to=${encodeURIComponent(page.url.pathname + page.url.search)}`);
		});
	});

	function switchContext(e: Event) {
		const id = (e.currentTarget as HTMLSelectElement).value;
		const c = contexts.find((x) => x.body_id === id);
		if (c) goto(bodyHref({ kind: c.body_kind, number: c.body_number }));
	}

	async function logout() {
		const idpLogout = await session.logout();
		if (idpLogout) window.location.assign(idpLogout);
		else goto('/login');
	}

	const permissions = $derived<string[]>(
		body?.permissions ?? contexts.find((c) => c.body_id === current?.id)?.permissions ?? []
	);
	// Kärnans menyval finns här. Modulernas kommer från servern (bara aktiva moduler som användaren får se).
	const moduleNav = $derived(body?.nav ?? contexts.find((c) => c.body_id === current?.id)?.nav ?? []);
	const nav = $derived(
		current
			? [
					{ href: bodyHref(current), label: 'Översikt', icon: '🏠' },
					{ href: bodyHref(current, 'medlemmar'), label: 'Medlemmar', icon: '👥' },
					...(['group', 'district', 'organisation'].includes(current.kind)
						? [{ href: bodyHref(current, 'vantelista'), label: 'Väntelista', icon: '⏳' }]
						: []),
					...(current.kind !== 'patrol' ? [{ href: bodyHref(current, 'organisation'), label: 'Organisation', icon: '🌳' }] : []),
					...moduleNav.map((n) => ({ href: bodyHref(current, n.segment), label: n.label, icon: n.icon })),
					...(permissions.includes('modules_manage') && !['troop', 'patrol', 'section'].includes(current.kind)
						? [{ href: bodyHref(current, 'moduler'), label: 'Moduler', icon: '⚙️' }]
						: [])
				]
			: []
	);
</script>

<svelte:head>
	<link rel="icon" type="image/png" href="/brand/Scoutsymbolen_rgb.png" />
</svelte:head>

{#if isLogin}
	{@render children()}
{:else if session.me}
	<div class="shell">
		<header class="topbar">
			<a class="brand" href="/">
				<img class="logo-img" src="/brand/scouterna_white_RGB.svg" alt="Scouterna" />
				<span class="product">Scoutnet <strong>2</strong></span>
				<span class="badge accent">prototyp</span>
			</a>
			{#if contexts.length > 0}
				<label class="ctx">
					<span class="muted">Du arbetar i</span>
					<select class="input" value={contextId} onchange={switchContext} aria-label="Byt enhet">
						{#each contexts as c (c.body_id)}
							<option value={c.body_id}>{c.body_name} – {c.roles.join(', ')}</option>
						{/each}
					</select>
				</label>
			{/if}
			<div class="spacer"></div>
			<a class="user" href="/mig" title="Min profil">
				<span class="avatar" aria-hidden="true">{session.me.name.slice(0, 1)}</span>
				<span>{session.me.name}</span>
			</a>
			<button class="btn ghost" onclick={logout}>Logga ut</button>
		</header>

		<aside class="sidebar" aria-label="Huvudmeny">
			<nav>
				{#each nav as item (item.label)}
					<a
						class="navitem"
						class:active={page.url.pathname === item.href || (item.label !== 'Översikt' && page.url.pathname.startsWith(`${item.href}/`))}
						href={item.href}
					>
						<span aria-hidden="true">{item.icon}</span>{item.label}
					</a>
				{/each}
				<hr />
				<a class="navitem" class:active={page.url.pathname === '/mig'} href="/mig"><span aria-hidden="true">🙂</span>Min profil</a>
				<a class="navitem" href="/docs" target="_blank" rel="noopener"><span aria-hidden="true">🧩</span>API-dokumentation</a>
			</nav>
			<p class="note muted">All data i prototypen är syntetisk. Personnummer är Skatteverkets testnummer.</p>
		</aside>

		<main class="content">
			{@render children()}
		</main>
	</div>
{:else}
	<div class="loading muted" aria-busy="true">Laddar…</div>
{/if}

<style>
	.shell {
		display: grid;
		grid-template-columns: 230px 1fr;
		grid-template-rows: 56px 1fr;
		min-height: 100vh;
	}
	.topbar {
		grid-column: 1 / -1;
		display: flex;
		align-items: center;
		gap: var(--space-4);
		padding: 0 var(--space-4);
		background: var(--sn-blue-800);
		color: #fff;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: #fff;
		font-size: 1.1rem;
		text-decoration: none;
	}
	.logo-img {
		height: 22px;
		width: auto;
		display: block;
	}
	.product {
		padding-left: var(--space-3);
		margin-left: var(--space-1);
		border-left: 1px solid rgb(255 255 255 / 35%);
	}
	.ctx {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: 0.85rem;
	}
	.ctx .muted {
		color: #c8d6e6;
	}
	.ctx select {
		background: var(--sn-blue-900);
		color: #fff;
		border-color: var(--sn-blue-600);
		max-width: 420px;
	}
	.spacer {
		flex: 1;
	}
	.user {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: #fff;
	}
	.avatar {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		border-radius: 50%;
		background: var(--sn-accent);
		color: var(--sn-blue-900);
		font-weight: 700;
	}
	.topbar :global(.btn.ghost) {
		color: #fff;
	}
	.sidebar {
		background: var(--surface);
		border-right: 1px solid var(--border);
		padding: var(--space-3);
		display: flex;
		flex-direction: column;
		justify-content: space-between;
	}
	nav {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.navitem {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: 9px 12px;
		border-radius: var(--radius-sm);
		color: var(--text);
		text-decoration: none;
	}
	.navitem:hover {
		background: var(--sn-blue-50);
	}
	.navitem.active {
		background: var(--sn-blue-50);
		color: var(--primary-strong);
		font-weight: 600;
		box-shadow: inset 3px 0 0 var(--primary);
	}
	hr {
		border: none;
		border-top: 1px solid var(--border);
		width: 100%;
	}
	.note {
		font-size: 0.78rem;
		padding: var(--space-2);
	}
	.content {
		padding: var(--space-5);
		min-width: 0;
	}
	.loading {
		display: grid;
		place-items: center;
		height: 100vh;
	}
	@media (max-width: 800px) {
		.shell {
			grid-template-columns: minmax(0, 1fr);
			grid-template-rows: auto auto 1fr;
		}
		.topbar {
			flex-wrap: wrap;
			padding: var(--space-2) var(--space-3);
			gap: var(--space-2) var(--space-3);
		}
		.ctx {
			order: 5;
			flex: 1 1 100%;
		}
		.ctx select {
			width: 100%;
			max-width: none;
		}
		.user span:not(.avatar) {
			display: none;
		}
		.ctx .muted {
			display: none;
		}
		.sidebar {
			border-right: none;
			border-bottom: 1px solid var(--border);
		}
		nav {
			flex-direction: row;
			overflow-x: auto;
		}
		.note,
		hr {
			display: none;
		}
		.content {
			padding: var(--space-3);
		}
	}
</style>
