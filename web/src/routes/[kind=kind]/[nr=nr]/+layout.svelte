<script lang="ts">
	import { page } from '$app/state';
	import { bodyHref } from '#lib/bodies.ts';
	import type { LayoutProps } from './$types';

	let { data, children }: LayoutProps = $props();

	const subpages: Record<string, string> = {
		medlemmar: 'Medlemmar',
		vantelista: 'Väntelista',
		organisation: 'Organisation'
	};
	const sub = $derived(page.route.id?.split('/').at(-1) ?? '');
	const subLabel = $derived(subpages[sub]);
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
			{#if subLabel}
				<a href={bodyHref(data.body)}>{data.body.name}</a>
			{:else}
				<span aria-current="page">{data.body.name}</span>
			{/if}
		</li>
		{#if subLabel}
			<li><span aria-current="page">{subLabel}</span></li>
		{/if}
	</ol>
</nav>

{@render children()}

<style>
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
