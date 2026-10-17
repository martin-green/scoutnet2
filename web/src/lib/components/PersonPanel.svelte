<script lang="ts">
	import {
		api,
		ApiError,
		addressTypeLabels,
		consentLabels,
		decidedByLabels,
		feeStatusLabels,
		fmtDate,
		fmtSek,
		invoiceDeliveryLabels,
		languageLabels,
		membershipStatusLabels,
		meritKinds,
		personStatusLabels,
		policeCheckLabels,
		sexLabels,
		troopTypeLabel,
		type PersonDetail
	} from '#lib/api/client.ts';
	import { bodyHref } from '#lib/bodies.ts';
	import AuditList from './AuditList.svelte';

	let { id, onclose, onopen }: { id: string; onclose: () => void; onopen?: (id: string) => void } = $props();

	let person = $state<PersonDetail | null>(null);
	let error = $state<string | null>(null);
	let revealed = $state<string | null>(null);
	let revealError = $state<string | null>(null);

	type Tab = 'oversikt' | 'familj' | 'medlemskap' | 'uppdrag' | 'meriter' | 'ovrigt' | 'logg';
	// Fliken behålls när man bläddrar mellan personer.
	let tab = $state<Tab>('oversikt');

	$effect(() => {
		const current = id;
		person = null;
		error = null;
		revealed = null;
		revealError = null;
		api.person(current)
			.then((p) => {
				if (current === id) person = p;
			})
			.catch((e) => (error = e instanceof ApiError && e.status === 404 ? 'Personen finns inte eller så saknar du behörighet.' : e.message));
	});

	async function reveal() {
		revealError = null;
		try {
			revealed = (await api.revealNationalId(id)).national_id;
			setTimeout(() => (revealed = null), 30_000);
		} catch (e) {
			revealError =
				e instanceof ApiError && e.problem?.code === 'step_up_required'
					? 'Kräver ny inloggning med MFA (step-up).'
					: e instanceof ApiError && e.status === 403
						? 'Din roll får inte visa personnummer.'
						: String(e);
		}
	}

	const can = (perm: string) => person?.permissions.includes(perm as never) ?? false;
	const yesNo = (b: boolean) => (b ? 'Ja' : 'Nej');

	const tabs = $derived.by(() => {
		if (!person) return [];
		const family = person.guardians.length + (person.household?.members.length ?? 0);
		const warn = person.police_check && ['missing', 'expired'].includes(person.police_check.status);
		const unpaid = person.fees?.some((f) => f.status !== 'paid');
		return [
			{ key: 'oversikt' as Tab, label: 'Översikt' },
			{ key: 'familj' as Tab, label: 'Familj', count: family || undefined },
			{ key: 'medlemskap' as Tab, label: 'Medlemskap', dot: unpaid },
			{ key: 'uppdrag' as Tab, label: 'Uppdrag', dot: warn },
			{ key: 'meriter' as Tab, label: 'Meriter', count: person.merits.length || undefined },
			{ key: 'ovrigt' as Tab, label: 'Inställningar' },
			...(can('audit_read') ? [{ key: 'logg' as Tab, label: 'Logg' }] : [])
		];
	});
	// Fliken Logg finns inte för alla; byt till översikten om den inte finns för nästa person.
	$effect(() => {
		if (person && !tabs.some((t) => t.key === tab)) tab = 'oversikt';
	});
</script>

<div class="backdrop" onclick={onclose} aria-hidden="true"></div>
<aside class="panel" aria-label="Persondetaljer">
	<button class="btn ghost close" onclick={onclose} aria-label="Stäng">✕</button>
	{#if error}
		<p class="muted">{error}</p>
	{:else if !person}
		<p class="muted">Laddar…</p>
	{:else}
		{@const ps = personStatusLabels[person.person_status]}
		<header>
			<div class="avatar" aria-hidden="true">{person.first_name[0]}{person.last_name[0]}</div>
			<div>
				<h2>{person.first_name} {person.last_name}{#if person.nickname}<span class="muted nick"> ”{person.nickname}”</span>{/if}</h2>
				<p class="muted">Medlemsnummer <span class="mono">{person.member_no}</span> · {person.age} år</p>
				<p class="badges">
					<span class="badge green">{membershipStatusLabels[person.membership.status] ?? person.membership.status}</span>
					{#if person.person_status !== 'active'}<span class="badge {ps?.tone}">{ps?.label ?? person.person_status}</span>{/if}
					{#if person.troop}<span class="badge blue">{person.troop.name}</span>{/if}
					{#if person.patrol}<span class="badge">{person.patrol.name}</span>{/if}
					{#if person.is_leader}<span class="badge accent">Ledare</span>{/if}
					{#if person.protected_identity}<span class="badge red">Skyddad identitet</span>{/if}
					{#if person.police_check && person.police_check.required && person.police_check.status !== 'valid'}
						{@const pc = policeCheckLabels[person.police_check.status]}
						<span class="badge {pc.tone}">Registerutdrag: {pc.label.toLowerCase()}</span>
					{/if}
				</p>
			</div>
		</header>

		<nav class="tabs" aria-label="Delar av personkortet">
			{#each tabs as t (t.key)}
				<button class="tab" class:on={tab === t.key} aria-current={tab === t.key ? 'page' : undefined} onclick={() => (tab = t.key)}>
					{t.label}
					{#if t.count}<span class="count">{t.count}</span>{/if}
					{#if t.dot}<span class="dot" title="Kräver åtgärd"></span>{/if}
				</button>
			{/each}
		</nav>

		{#if tab === 'oversikt'}
			<section>
				<h3>Kontakt</h3>
				{#if person.contact}
					{@const c = person.contact}
					<dl>
						<dt>E-post</dt>
						<dd>{#if c.email}<a href={`mailto:${c.email}`}>{c.email}</a>{:else}–{/if}</dd>
						{#if c.alt_email}
							<dt>Alternativ e-post</dt>
							<dd><a href={`mailto:${c.alt_email}`}>{c.alt_email}</a></dd>
						{/if}
						{#if c.scouterna_email}
							<dt>Scouterna-e-post</dt>
							<dd><a href={`mailto:${c.scouterna_email}`}>{c.scouterna_email}</a></dd>
						{/if}
						<dt>Mobil</dt>
						<dd>{#if c.mobile_phone}<a href={`tel:${c.mobile_phone.replaceAll(' ', '')}`}>{c.mobile_phone}</a>{:else}–{/if}</dd>
						{#if c.home_phone}
							<dt>Hemtelefon</dt>
							<dd>{c.home_phone}</dd>
						{/if}
					</dl>
				{:else}
					<p class="locked">🔒 Din roll ger inte tillgång till kontaktuppgifter.</p>
				{/if}
			</section>

			<section>
				<h3>Personuppgifter</h3>
				<dl>
					<dt>Född</dt>
					<dd>{fmtDate(person.birth_date)}</dd>
					<dt>Kön</dt>
					<dd>{sexLabels[person.sex] ?? person.sex}</dd>
					<dt>Personnummer</dt>
					<dd>
						{#if revealed}
							<span class="mono revealed">{revealed}</span> <span class="muted small">(visas 30 s, loggat)</span>
						{:else if person.national_id_masked}
							<span class="mono">{person.national_id_masked}</span>
							{#if can('national_id_read')}
								<button class="btn small" onclick={reveal} title="Visningen loggas i revisionsloggen">Visa</button>
							{/if}
						{:else}
							<span class="locked">🔒 dolt</span>
						{/if}
						{#if revealError}<p class="error small">{revealError}</p>{/if}
					</dd>
					<dt>Språk</dt>
					<dd>{languageLabels[person.preferred_language] ?? person.preferred_language}</dd>
					{#if person.addresses}
						{#each person.addresses as a, i (i)}
							<dt>{addressTypeLabels[a.address_type] ?? a.address_type}</dt>
							<dd>
								{#if a.care_of}c/o {a.care_of}<br />{/if}
								{a.line1}{#if a.line2}<br />{a.line2}{/if}{#if a.line3}<br />{a.line3}{/if}<br />
								{a.postcode} {a.city}{#if a.country_code !== 'SE'}, {a.country_code}{/if}
							</dd>
						{:else}
							<dt>Adress</dt>
							<dd>–</dd>
						{/each}
						{#if person.protected_identity}
							<dt></dt>
							<dd class="muted small">Skyddad identitet: endast förmedlingsadress lagras.</dd>
						{/if}
					{:else}
						<dt>Adress</dt>
						<dd class="locked">🔒 dold</dd>
					{/if}
					{#if person.can_help}
						<dt>Engagemang</dt>
						<dd><span class="badge blue">Vill hjälpa till som ledare</span></dd>
					{/if}
				</dl>
			</section>

			{#if person.presentation}
				<section>
					<h3>Presentation</h3>
					<p class="pre">{person.presentation}</p>
				</section>
			{/if}

			<section>
				<h3>Närvaro {person.attendance.term}</h3>
				{#if person.attendance.total > 0}
					<p>
						<strong>{person.attendance.present}</strong> av {person.attendance.total} sammankomster
						({Math.round((person.attendance.present / person.attendance.total) * 100)} %)
					</p>
				{:else}
					<p class="muted">Inga registrerade sammankomster ännu.</p>
				{/if}
			</section>
		{:else if tab === 'familj'}
			<section>
				<h3>Anhöriga</h3>
				{#if person.guardians.length > 0}
					<ul class="list">
						{#each person.guardians as g (g.position + g.name)}
							<li>
								<span class="muted small">Anhörig {g.position} · {g.relation}</span><br />
								<strong>{g.name}</strong>
								{#if g.has_account}<span class="badge green" title="Har eget konto i Scoutnet 2">konto</span>{/if}
								<br />
								{#if g.email}<a href={`mailto:${g.email}`}>{g.email}</a>{:else if person.contact}<span class="badge red">saknar e-post</span>{/if}
								{#if g.mobile_phone}<span class="muted"> · mobil {g.mobile_phone}</span>{/if}
								{#if g.home_phone}<span class="muted"> · hem {g.home_phone}</span>{/if}
								{#if !person.contact}<span class="locked small">🔒 kontaktuppgifter dolda</span>{/if}
							</li>
						{/each}
					</ul>
				{:else}
					<p class="muted">Inga anhöriga registrerade.</p>
				{/if}
			</section>
			<section>
				<h3>Hushåll</h3>
				{#if person.household}
					<p><strong>{person.household.name}</strong></p>
					{#if person.household.members.length > 0}
						<ul class="list">
							{#each person.household.members as m (m.id)}
								<li>
									{#if onopen}
										<button class="link" onclick={() => onopen(m.id)}>{m.first_name} {m.last_name}</button>
									{:else}
										{m.first_name} {m.last_name}
									{/if}
									<span class="muted">· {m.age} år{m.group_name ? ` · ${m.group_name}` : ''}</span>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="muted small">Inga andra hushållsmedlemmar som du har behörighet att se.</p>
					{/if}
				{:else}
					<p class="muted">Inte kopplad till något hushåll.</p>
				{/if}
			</section>
		{:else if tab === 'medlemskap'}
			{@const m = person.membership}
			<section>
				<h3>Medlemskap</h3>
				<dl>
					<dt>Kår</dt>
					<dd>{m.group.name}</dd>
					<dt>Status</dt>
					<dd>{membershipStatusLabels[m.status] ?? m.status}</dd>
					<dt>Medlemsstatus</dt>
					<dd><span class="badge {ps?.tone}">{ps?.label ?? person.person_status}</span></dd>
					<dt>Medlem sedan</dt>
					<dd>{fmtDate(m.member_since)}</dd>
					<dt>Godkänd av kåren</dt>
					<dd>{fmtDate(m.confirmed_at)}</dd>
					{#if m.waiting_since}
						<dt>På väntelista sedan</dt>
						<dd>{fmtDate(m.waiting_since)}{#if m.wished_troop_type} · önskar {troopTypeLabel(m.wished_troop_type).toLowerCase()}{/if}</dd>
					{/if}
					<dt>Avgiftsgrupp</dt>
					<dd>{#if person.fees}{m.fee_group ?? 'Standard'}{:else}<span class="locked">🔒 dold</span>{/if}</dd>
					<dt>Uppgifter bekräftade</dt>
					<dd>{fmtDate(person.confirmed_at)}</dd>
					<dt>Registrerad</dt>
					<dd>{fmtDate(person.created_at)}</dd>
				</dl>
			</section>
			<section>
				<h3>Terminsavgifter</h3>
				{#if person.fees}
					{#if person.fees.length > 0}
						<table class="mini">
							<thead><tr><th>Termin</th><th class="right">Belopp</th><th>Förfaller</th><th>OCR</th><th>Status</th></tr></thead>
							<tbody>
								{#each person.fees as f (f.term_id + f.group.id)}
									{@const s = feeStatusLabels[f.status]}
									<tr>
										<td>{f.term}{#if f.fee_group}<br /><span class="muted small">{f.fee_group}</span>{/if}</td>
										<td class="right">{fmtSek(f.amount_ore)}</td>
										<td>{fmtDate(f.due_date)}</td>
										<td class="mono small">{f.kid ?? '–'}</td>
										<td><span class="badge {s.tone}">{s.label}</span>{#if f.paid_on}<br /><span class="muted small">{fmtDate(f.paid_on)}</span>{/if}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					{:else}
						<p class="muted">Inga avgifter.</p>
					{/if}
				{:else}
					<p class="locked">🔒 Kräver behörighet att se avgifter (t.ex. kassör).</p>
				{/if}
			</section>
		{:else if tab === 'uppdrag'}
			<section>
				<h3>Registerutdrag</h3>
				{#if person.police_check}
					{@const pc = person.police_check}
					{@const l = policeCheckLabels[pc.status]}
					<dl>
						<dt>Status</dt>
						<dd><span class="badge {l.tone}">{l.label}</span>{#if pc.forced}<span class="muted small"> · krävs särskilt (tvingat)</span>{/if}</dd>
						{#if pc.shown_on}
							<dt>Senast uppvisat</dt>
							<dd>{fmtDate(pc.shown_on)} <span class="muted small">(utfärdat {fmtDate(pc.issued_on)})</span></dd>
							<dt>Giltigt till</dt>
							<dd>{fmtDate(pc.valid_until)}</dd>
						{/if}
					</dl>
					{#if pc.history.length > 1}
						<details>
							<summary class="small">Tidigare utdrag ({pc.history.length - 1})</summary>
							<ul class="list small">
								{#each pc.history.slice(1) as h (h.shown_on)}
									<li>Uppvisat {fmtDate(h.shown_on)}{h.verified_by ? ` för ${h.verified_by}` : ''}{h.body_name ? `, ${h.body_name}` : ''}</li>
								{/each}
							</ul>
						</details>
					{/if}
					<p class="muted small">Endast att utdraget visats registreras – aldrig innehållet.</p>
				{:else}
					<p class="locked">🔒 Din roll ger inte tillgång till registerutdrag.</p>
				{/if}
			</section>
			<section>
				<h3>Uppföljningssamtal</h3>
				{#if person.follow_ups}
					{#if person.follow_ups.length > 0}
						<ul class="list">
							{#each person.follow_ups as f, i (i)}
								<li>
									{fmtDate(f.held_on)}{#if f.role_label} · {f.role_label}{/if}{#if f.body_name} <span class="muted">– {f.body_name}</span>{/if}
									{#if f.registered_by}<br /><span class="muted small">Registrerat av {f.registered_by}</span>{/if}
								</li>
							{/each}
						</ul>
					{:else}
						<p class="muted">Inga uppföljningssamtal registrerade.</p>
					{/if}
				{:else}
					<p class="locked">🔒 Dolt för din roll.</p>
				{/if}
			</section>
			<section>
				<h3>Roller</h3>
				{#if person.roles.length > 0}
					<ul class="list">
						{#each person.roles as r (r.body_id + r.role_key)}
							<li>{r.role_label} <span class="muted">– <a href={bodyHref({ kind: r.body_kind, number: r.body_number })}>{r.body_name}</a></span></li>
						{/each}
					</ul>
				{:else}
					<p class="muted">Inga aktuella roller.</p>
				{/if}
				{#if person.role_history.length > 0}
					<details>
						<summary class="small">Rollhistorik ({person.role_history.length})</summary>
						<ul class="list small">
							{#each person.role_history as r, i (i)}
								<li>{r.role_label} <span class="muted">– <a href={bodyHref(r.body)}>{r.body.name}</a>, {fmtDate(r.from)}–{fmtDate(r.to)}</span></li>
							{/each}
						</ul>
					</details>
				{/if}
			</section>
			<section>
				<h3>Placeringar</h3>
				<ul class="list">
					{#each person.placements as p (p.body.id + p.from)}
						<li>
							<a href={bodyHref(p.body)}>{p.body.name}</a>
							{#if p.is_leader}<span class="badge accent">ledare</span>{/if}
							<span class="muted">{p.to ? `${fmtDate(p.from)}–${fmtDate(p.to)}` : `sedan ${fmtDate(p.from)}`}</span>
						</li>
					{/each}
				</ul>
			</section>
		{:else if tab === 'meriter'}
			{#each meritKinds as k (k.key)}
				{@const items = person.merits.filter((m) => m.kind === k.key)}
				{#if items.length > 0}
					<section>
						<h3>{k.label}</h3>
						<ul class="list">
							{#each items as m, i (i)}
								<li>
									<strong>{m.name}</strong> <span class="muted">· {fmtDate(m.completed_on)}</span>
									{#if m.issued_by || m.organizer || m.place}
										<br /><span class="muted small">{[m.issued_by, m.organizer, m.place].filter(Boolean).join(' · ')}</span>
									{/if}
									{#if m.note}<br /><span class="small">{m.note}</span>{/if}
								</li>
							{/each}
						</ul>
					</section>
				{/if}
			{/each}
			{#if person.merits.length === 0}
				<p class="muted">Inga registrerade utmärkelser, märken eller utbildningar.</p>
			{/if}
		{:else if tab === 'ovrigt'}
			<section>
				<h3>Samtycken</h3>
				{#if person.consents.length > 0}
					<ul class="list">
						{#each person.consents as c (c.kind)}
							<li>
								<span class="badge {c.granted ? 'green' : 'red'}">{c.granted ? '✓ Ja' : '✗ Nej'}</span>
								{consentLabels[c.kind] ?? c.kind}
								<span class="muted small">· {decidedByLabels[c.decided_by] ?? c.decided_by}, {fmtDate(c.decided_at)}</span>
							</li>
						{/each}
					</ul>
				{:else}
					<p class="muted">Inga samtycken registrerade – utgå från nej.</p>
				{/if}
			</section>
			<section>
				<h3>Utskick och fakturor</h3>
				{#if person.preferences}
					{@const pr = person.preferences}
					<dl>
						<dt>Nyhetsbrev</dt>
						<dd>{yesNo(pr.newsletter)}</dd>
						<dt>Medlemstidning</dt>
						<dd>{yesNo(pr.magazine)}</dd>
						<dt>Utskick till alt. e-post</dt>
						<dd>{yesNo(pr.mailing_to_alt_email)}</dd>
						<dt>Adress från SPAR</dt>
						<dd>{yesNo(pr.spar_sync)}</dd>
						<dt>Faktura medlemsavgift</dt>
						<dd>{invoiceDeliveryLabels[pr.invoice_delivery_membership] ?? pr.invoice_delivery_membership}</dd>
						<dt>Faktura arrangemang</dt>
						<dd>{invoiceDeliveryLabels[pr.invoice_delivery_events] ?? pr.invoice_delivery_events}</dd>
					</dl>
				{:else}
					<p class="locked">🔒 Dolt för din roll.</p>
				{/if}
			</section>
			{#if person.custom_fields.length > 0}
				<section>
					<h3>Kårens egna fält</h3>
					<dl>
						{#each person.custom_fields as f (f.key)}
							<dt title={f.body_name}>{f.label}</dt>
							<dd>{f.value}</dd>
						{/each}
					</dl>
				</section>
			{/if}
			{#if person.note}
				<section>
					<h3>Intern anteckning</h3>
					<p class="pre note">{person.note}</p>
					<p class="muted small">Synlig för medlemsregistrerare och kårordförande. Får inte innehålla hälsouppgifter.</p>
				</section>
			{/if}
		{:else if tab === 'logg'}
			<section>
				<h3>Vem har sett och ändrat uppgifterna</h3>
				<AuditList personId={person.id} />
			</section>
		{/if}

		<p class="muted small audit">👁 Den här visningen har registrerats i revisionsloggen.</p>
	{/if}
</aside>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 20%);
		z-index: 40;
	}
	.panel {
		position: fixed;
		top: 0;
		right: 0;
		bottom: 0;
		width: min(560px, 100vw);
		background: var(--surface);
		box-shadow: -8px 0 24px rgb(0 0 0 / 15%);
		z-index: 50;
		overflow-y: auto;
		padding: var(--space-5);
		animation: slide 0.15s ease-out;
	}
	@keyframes slide {
		from {
			transform: translateX(30px);
			opacity: 0;
		}
	}
	.close {
		position: absolute;
		top: var(--space-3);
		right: var(--space-3);
	}
	header {
		display: flex;
		gap: var(--space-4);
		align-items: flex-start;
		margin-bottom: var(--space-3);
	}
	header h2 {
		margin: 0;
	}
	header p {
		margin: 4px 0;
	}
	.nick {
		font-weight: 400;
		font-size: 0.9em;
	}
	.avatar {
		flex: none;
		display: grid;
		place-items: center;
		width: 56px;
		height: 56px;
		border-radius: 50%;
		background: var(--sn-blue-50);
		color: var(--primary-strong);
		font-weight: 700;
		font-size: 1.2rem;
	}
	.badges {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.tabs {
		display: flex;
		gap: 0;
		overflow-x: auto;
		scrollbar-width: none;
		border-bottom: 1px solid var(--border);
		margin: 0 calc(-1 * var(--space-5)) var(--space-2);
		padding: 0 var(--space-5);
		position: sticky;
		top: calc(-1 * var(--space-5));
		background: var(--surface);
		z-index: 1;
	}
	.tab {
		border: 0;
		background: none;
		padding: 10px 6px 8px;
		font: inherit;
		font-size: 0.9rem;
		color: var(--muted);
		border-bottom: 3px solid transparent;
		cursor: pointer;
		white-space: nowrap;
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}
	.tab:hover {
		color: var(--text);
	}
	.tab.on {
		color: var(--primary-strong);
		border-bottom-color: var(--primary);
		font-weight: 600;
	}
	.count {
		background: var(--sn-gray-100);
		color: var(--muted);
		border-radius: 999px;
		padding: 0 6px;
		font-size: 0.75rem;
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--sn-red);
	}
	section {
		border-top: 1px solid var(--border);
		padding: var(--space-3) 0;
	}
	.tabs + section {
		border-top: 0;
	}
	h3 {
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--muted);
		margin: 0 0 var(--space-2);
	}
	dl {
		display: grid;
		grid-template-columns: 150px 1fr;
		gap: 6px var(--space-3);
		margin: 0;
	}
	dt {
		color: var(--muted);
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: grid;
		gap: 8px;
	}
	details {
		margin-top: var(--space-2);
	}
	summary {
		cursor: pointer;
		color: var(--primary);
		margin-bottom: 6px;
	}
	.mini {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.88rem;
	}
	.mini th {
		text-align: left;
		color: var(--muted);
		font-weight: 500;
		border-bottom: 1px solid var(--border);
		padding: 4px 6px 4px 0;
	}
	.mini td {
		padding: 6px 6px 6px 0;
		border-bottom: 1px solid var(--border);
		vertical-align: top;
	}
	.right {
		text-align: right;
	}
	.pre {
		white-space: pre-wrap;
		margin: 0;
	}
	.note {
		background: var(--sn-accent-50);
		padding: var(--space-2) var(--space-3);
		border-radius: 6px;
		margin-bottom: 6px;
	}
	.link {
		border: 0;
		background: none;
		padding: 0;
		font: inherit;
		color: var(--primary);
		cursor: pointer;
		text-decoration: underline;
	}
	.locked {
		color: var(--muted);
		margin: 0;
	}
	.small {
		font-size: 0.82rem;
	}
	.btn.small {
		padding: 2px 8px;
		margin-left: 6px;
	}
	.revealed {
		background: var(--sn-accent-50);
		padding: 2px 6px;
		border-radius: 4px;
	}
	.error {
		color: var(--sn-red);
		margin: 4px 0 0;
	}
	.audit {
		margin-top: var(--space-4);
	}
</style>
