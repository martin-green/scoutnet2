<script lang="ts">
	import { page } from '$app/state';
	import { bodyHref } from '#lib/bodies.ts';
	import type { LayoutProps } from './$types';

	let { data, children }: LayoutProps = $props();

	const subpages: Record<string, string> = {
		medlemmar: 'Medlemmar',
		vantelista: 'Väntelista',
		organisation: 'Organisation',
		narvaro: 'Närvaro',
		bidrag: 'Bidrag',
		integrationer: 'Integrationer',
		statistik: 'Statistik',
		moduler: 'Moduler'
	};
	// Undersidor efter /[kind]/[nr]. Dynamiska delar (t.ex. ett möte) får sin etikett från sidans data (`crumb`).
	const crumbs = $derived.by(() => {
		const segments = (page.route.id ?? '').split('/').slice(3);
		const out: { label: string; href: string }[] = [];
		let path = '';
		for (const s of segments) {
			path = path ? `${path}/${s}` : s;
			if (s.startsWith('[')) {
				const label = (page.data as { crumb?: string }).crumb;
				if (label) out.push({ label, href: page.url.pathname });
			} else if (subpages[s]) {
				out.push({ label: subpages[s], href: bodyHref(data.body, path) });
			}
		}
		return out;
	});
	const subLabel = $derived(crumbs.at(-1)?.label);

	// En undersida som hör till en modul som är av för enheten visas inte. Länkar och bokmärken
	// leder i stället till en förklaring.
	const segment = $derived((page.route.id ?? '').split('/')[3] ?? '');
	const inactive = $derived(
		data.catalog.find((m) => m.nav.some((n) => n.segment === segment) && !data.body.modules.includes(m.id))
	);
	const canManageModules = $derived(
		data.body.permissions.includes('modules_manage') && !['troop', 'patrol', 'section'].includes(data.body.kind)
	);
</script>

<svelte:head>
	<title>{subLabel ? `${subLabel} – ` : ''}{data.body.name} – Scoutnet 2</title>
</svelte:head>

<nav class="crumbs" aria-label="Brödsmulor">
	<ol>
		{#each data.body.ancestors as a (a.id)}
			<li>
				{#if a.accessible}
					<a href={bodyHref(a)}>{a.name}</a>
				{:else}
					<span class="muted" title="Du har ingen roll här">{a.name}</span>
				{/if}
			</li>
		{/each}
		<li>
			{#if crumbs.length > 0}
				<a href={bodyHref(data.body)}>{data.body.name}</a>
			{:else}
				<span aria-current="page">{data.body.name}</span>
			{/if}
		</li>
		{#each crumbs as c, i (c.href)}
			<li>
				{#if i < crumbs.length - 1}
					<a href={c.href}>{c.label}</a>
				{:else}
					<span aria-current="page">{c.label}</span>
				{/if}
			</li>
		{/each}
	</ol>
</nav>

{#if inactive}
	<section class="card off">
		<h1><span aria-hidden="true">{inactive.icon}</span> {inactive.name} är inte aktiverad</h1>
		<p>Modulen {inactive.name} är av för {data.body.name}. {inactive.description}</p>
		{#if canManageModules}
			<a class="btn primary" href={bodyHref(data.body, 'moduler')}>Gå till Moduler</a>
		{:else}
			<p class="muted">Kårens ordförande, eller distriktet eller förbundet, väljer vilka moduler som används.</p>
		{/if}
	</section>
{:else}
	{@render children()}
{/if}

<style>
	.off {
		max-width: 640px;
	}
	.off h1 {
		font-size: 1.4rem;
		margin-top: 0;
	}
	.crumbs {
		font-size: 0.85rem;
		margin-bottom: var(--space-2);
	}
	ol {
		display: flex;
		flex-wrap: wrap;
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li + li::before {
		content: '›';
		color: var(--muted);
		padding: 0 0.45em;
	}
	a {
		color: var(--primary);
		text-decoration: none;
	}
	a:hover {
		text-decoration: underline;
	}
	[aria-current='page'] {
		color: var(--text);
		font-weight: 600;
	}
</style>
