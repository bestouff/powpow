/* Volunteer guides. Highlighted links and controls remain interactive. */
"use strict";

function initVolunteerTutorial(prefix, viewer = null) {
  const root = document.getElementById("volunteer-tutorial");
  const help = document.getElementById("volunteer-help");
  if (!root || !help || root.dataset.ready) return;
  root.dataset.ready = "true";
  root.dataset.audience = viewer ? "member" : "anonymous";
  help.hidden = false;

  const card = root.querySelector(".tutorial-card");
  const drawing = root.querySelector("svg");
  const login = document.getElementById("login-btn");
  const search = document.getElementById("login-search");
  const membership = document.getElementById("membership-link");
  const send = document.getElementById("send-btn");
  const success = document.getElementById("success-box");
  const isOwnPage = viewer && document.getElementById("person-data")?.dataset.staffId === viewer.id;
  const ateliers = isOwnPage ? document.getElementById("person-ateliers") : null;
  const planning = isOwnPage ? document.getElementById("person-calendar") : null;
  const qualifications = isOwnPage ? document.getElementById("person-qualifications") : null;
  const memberSteps = ["profile", "ateliers", "planning", "qualifications"];
  const burger = document.querySelector(".navbar-burger");
  const menu = document.getElementById("main-navbar");
  const missing = document.getElementById("tutorial-missing");
  const back = document.getElementById("tutorial-back");
  const dismiss = document.getElementById("tutorial-dismiss");
  const navigation = root.querySelector(".tutorial-navigation");
  const previous = document.getElementById("tutorial-previous");
  const nextButton = document.getElementById("tutorial-next");
  const key = "powpow:volunteer-guide:v1:" + prefix + (viewer ? ":member:" + viewer.id : "");
  let memory = "";
  let step = "";
  let target = null;
  let openedMenu = false;
  let pendingLayout = false;
  let described = null;
  let restartHintTimer = null;

  function clearDescription() {
    if (!described) return;
    const ids = (described.getAttribute("aria-describedby") || "").split(/\s+/).filter(id => id && id !== "tutorial-description");
    if (ids.length) described.setAttribute("aria-describedby", ids.join(" "));
    else described.removeAttribute("aria-describedby");
    described = null;
  }

  function revealNavigationTarget() {
    if (!["login", "profile", "restart"].includes(step) || target.getClientRects().length || !menu || !burger) return;
    openedMenu = true;
    menu.classList.add("is-active");
    burger.classList.add("is-active");
    burger.setAttribute("aria-expanded", "true");
  }

  function savedStep() {
    try { return localStorage.getItem(key) || memory; }
    catch (_) { return memory; }
  }

  function remember(value) {
    memory = value;
    try { localStorage.setItem(key, value); }
    catch (_) { /* The guide also works when browser storage is disabled. */ }
  }

  function hide() {
    clearTimeout(restartHintTimer);
    restartHintTimer = null;
    const restoreFocus = root.contains(document.activeElement) || (openedMenu && menu.contains(document.activeElement));
    root.hidden = true;
    clearDescription();
    target?.classList.remove("tutorial-target");
    document.body.classList.remove("tutorial-active");
    document.body.style.removeProperty("--tutorial-space");
    document.body.style.removeProperty("--tutorial-results-height");
    document.body.style.removeProperty("--tutorial-target-height");
    if (openedMenu) {
      menu.classList.remove("is-active");
      burger.classList.remove("is-active");
      burger.setAttribute("aria-expanded", "false");
      openedMenu = false;
    }
    if (restoreFocus) (help.getClientRects().length ? help : burger)?.focus({preventScroll: true});
  }

  function finish() {
    const wasHint = step === "restart";
    remember("done");
    hide();
    if (!wasHint) {
      show("restart");
      restartHintTimer = setTimeout(hide, 3000);
    }
  }

  function loginStep() {
    if (success?.getClientRects().length) return "inbox";
    if (send?.getClientRects().length) return "send";
    return "search";
  }

  function isGuiding() {
    return !root.hidden && step !== "restart";
  }

  function show(next, focus = false) {
    clearTimeout(restartHintTimer);
    restartHintTimer = null;
    target?.classList.remove("tutorial-target");
    step = next;
    target = {login, search, membership, send, inbox: success, profile: login, ateliers, planning, qualifications, restart: help}[step];
    if (!target) return;
    target.classList.add("tutorial-target");
    if (target.classList.contains("tutorial-section")) target.scrollTop = 0;
    if (step !== "restart") remember(step);
    revealNavigationTarget();
    clearDescription();
    described = step === "search" ? document.getElementById("search-input") : target;
    described.setAttribute("aria-describedby", ((described.getAttribute("aria-describedby") || "") + " tutorial-description").trim());
    const entity = success?.dataset.entityName.trim();
    const email = entity ? `l’email envoyé par « ${entity} »` : "l’email de connexion";
    const copy = {
      login: ["1", "Vous êtes bénévole ?", "Cliquez sur « Se connecter » pour accéder à votre espace bénévole et vous inscrire au planning."],
      search: ["2", "Retrouvez votre nom", "Saisissez au moins 4 caractères de votre prénom ou de votre nom de famille (un morceau suffit), puis cliquez sur votre nom dans la liste qui apparaît."],
      membership: ["3", "Rejoignez les bénévoles !", "Pour devenir bénévole, prenez votre adhésion avec ce lien HelloAsso. Revenez ensuite rechercher votre nom, une fois votre adhésion enregistrée. Si vous avez déjà adhéré, réessayez un peu plus tard ou contactez l’association."],
      send: ["4", "Recevez votre lien de connexion", "Votre nom est sélectionné. Cliquez sur « Envoyer le lien de connexion » pour recevoir l’email qui vous permettra de vous connecter."],
      inbox: ["5", "Consultez votre boîte mail", `Ouvrez votre boîte de réception, repérez ${email} et cliquez sur le lien qu’il contient pour vous connecter. Pensez aussi à vérifier les courriers indésirables.`],
      profile: ["1", "Votre page personnelle", "Cliquez sur votre nom pour ouvrir votre page personnelle. Ce lien reste accessible dans le menu : vous pouvez y revenir à tout moment."],
      ateliers: ["2", "Choisissez vos ateliers", "Cochez les rôles auxquels vous souhaitez participer. Pour les ateliers marqués « Validation requise », vous devrez attendre que le chef valide votre participation."],
      planning: ["3", "Indiquez vos disponibilités", "Cochez les demi-journées auxquelles vous souhaitez participer : matin, après-midi, ou les deux. Pour les activités nocturnes, choisissez soirée ou nuit." + (planning && !planning.querySelector(".pcal-table") ? " Aucun créneau n’est encore disponible : choisissez vos ateliers et revenez une fois leurs plannings publiés." : "")],
      qualifications: ["4", "Déclarez vos qualifications", "Certains rôles demandent des qualifications. Dans « Formations », choisissez vos qualifications et leur date d’obtention, puis cliquez sur « Ajouter »." + (qualifications && !qualifications.querySelector("#pq-qual") ? " Si votre qualification n’est pas proposée, contactez votre chef d’atelier." : "")],
      restart: ["", "Le guide reste disponible", "Cliquez sur « Aide » pour relancer le guide à tout moment."],
    }[step];
    root.dataset.step = step;
    const progress = root.querySelector(".tutorial-progress");
    progress.hidden = step === "restart";
    progress.textContent = step === "restart" ? "" : "Le guide bénévole · " + copy[0] + (viewer ? "/4" : "/5");
    document.getElementById("tutorial-title").textContent = copy[1];
    document.getElementById("tutorial-description").textContent = copy[2];
    card.querySelector("[aria-live]").scrollTop = 0;
    missing.hidden = step !== "search";
    back.hidden = step !== "membership";
    navigation.hidden = !viewer || step === "restart";
    previous.hidden = step === "profile";
    nextButton.textContent = step === "profile" ? "Ma page personnelle" : step === "qualifications" ? "Terminer" : "Suivant";
    dismiss.hidden = step === "restart";
    dismiss.textContent = step === "login" ? "Je ne souhaite pas être bénévole" : step === "inbox" ? "J’ai compris" : "Fermer le guide";
    root.hidden = false;
    document.body.classList.add("tutorial-active");
    layout();
    reveal();
    if (focus) (step === "search" ? document.getElementById("search-input") : target).focus({preventScroll: true});
  }

  function viewport() {
    return {top: window.visualViewport?.offsetTop || 0, height: window.visualViewport?.height || window.innerHeight};
  }

  function reveal() {
    if (root.hidden || !target) return;
    const rect = target.getBoundingClientRect();
    const view = viewport();
    const bottom = card.getBoundingClientRect().top;
    const docked = card.dataset.docked === "true";
    if (rect.top < view.top + 12 || rect.bottom > (docked ? bottom - 24 : view.top + view.height - 24)) {
      window.scrollBy({top: rect.top - view.top - 32, behavior: "instant"});
    }
    scheduleLayout();
  }

  function scheduleLayout() {
    if (pendingLayout || root.hidden) return;
    pendingLayout = true;
    requestAnimationFrame(() => { pendingLayout = false; layout(); });
  }

  function layout() {
    if (root.hidden || !target) return;
    const width = document.documentElement.clientWidth;
    const view = viewport();
    let rect = target.getBoundingClientRect();
    card.classList.toggle("is-compact", view.height < 500);
    const cw = card.offsetWidth;
    const clamp = (value, low, high) => Math.max(low, Math.min(value, high));
    const sideRoom = Math.max(rect.left, width - rect.right);
    const docked = width < 900 || sideRoom < cw + 80;
    card.style.maxHeight = viewer && docked ? view.height * (view.height < 500 ? .42 : .5) + "px" : "";
    const ch = card.offsetHeight;
    card.dataset.docked = String(docked);
    // Keep long profile sections scrollable above the card on small screens.
    document.body.style.setProperty("--tutorial-target-height", Math.max(100, view.height - (docked ? ch + 90 : 64)) + "px");
    rect = target.getBoundingClientRect();
    let left, top;
    if (docked) {
      left = (width - cw) / 2;
      top = view.top + view.height - ch - 16;
    } else {
      left = rect.left > cw + 80 ? rect.left - cw - 64 : rect.right + 64;
      top = ["login", "profile", "restart"].includes(step) ? rect.bottom + 100 : rect.top + 20;
    }
    card.style.left = clamp(left, 16, width - cw - 16) + "px";
    card.style.top = clamp(top, view.top + 16, view.top + view.height - ch - 16) + "px";
    document.body.style.setProperty("--tutorial-space", docked ? ch + 70 + "px" : "0px");
    document.body.style.setProperty("--tutorial-results-height", Math.max(64, view.height - ch - 190) + "px");

    drawing.setAttribute("viewBox", "0 0 " + width + " " + window.innerHeight);
    drawing.style.display = !target.getClientRects().length || rect.bottom < view.top || rect.top > view.top + view.height ? "none" : "";
    const x = Math.max(5, rect.left - 12);
    const y = Math.max(view.top + 5, rect.top - 14);
    const w = Math.min(width - 5, rect.right + 12) - x;
    const h = Math.max(30, rect.bottom + 14 - y);
    const radius = Math.min(26, h * .35, w * .18);
    // Uneven Bézier loops and an open arrowhead give the red pen its hand-drawn shape.
    root.querySelector(".tutorial-circle").setAttribute("d",
      `M ${x + w * .5} ${y + 2} C ${x + w * .7} ${y - 3}, ${x + w * .8} ${y - 2}, ${x + w - radius} ${y} Q ${x + w + 3} ${y - 1}, ${x + w} ${y + radius} C ${x + w + 3} ${y + h * .5}, ${x + w - 2} ${y + h * .7}, ${x + w} ${y + h - radius} Q ${x + w + 2} ${y + h + 3}, ${x + w - radius} ${y + h} C ${x + w * .6} ${y + h + 5}, ${x + w * .3} ${y + h - 2}, ${x + radius} ${y + h} Q ${x - 3} ${y + h + 2}, ${x} ${y + h - radius} C ${x - 3} ${y + h * .5}, ${x + 2} ${y + h * .3}, ${x} ${y + radius} Q ${x - 1} ${y - 3}, ${x + radius} ${y} C ${x + w * .25} ${y - 3}, ${x + w * .4} ${y - 1}, ${x + w * .56} ${y + 3}`);
    root.querySelector(".tutorial-circle-echo").setAttribute("d",
      `M ${x + w * .17} ${y - 2} C ${x + w * .45} ${y - 7}, ${x + w * .75} ${y + 1}, ${x + w - radius} ${y - 3} Q ${x + w + 5} ${y - 3}, ${x + w + 3} ${y + radius} L ${x + w + 2} ${y + h * .65}`);
    const box = card.getBoundingClientRect();
    let sx, sy, ex, ey, cx, cy;
    if (docked || box.top > rect.bottom + 30) {
      sx = box.left + box.width * .65; sy = box.top - 8;
      ex = x + w * .68; ey = y + h + 9;
      const bend = Math.min(55, Math.abs(sy - ey) * .45);
      cx = ex + bend * .8; cy = ey + bend;
    } else if (box.right < rect.left) {
      sx = box.right + 8; sy = box.top + 48;
      ex = x - 9; ey = y + h * .55;
      cx = ex - 35; cy = ey - 30;
    } else {
      sx = box.left - 8; sy = box.top + 48;
      ex = x + w + 9; ey = y + h * .55;
      cx = ex + 35; cy = ey - 30;
    }
    root.querySelector(".tutorial-arrow").setAttribute("d", `M ${sx} ${sy} C ${sx + (ex - sx) * .3} ${sy - Math.min(45, Math.abs(sy - ey) * .4)}, ${cx} ${cy}, ${ex} ${ey}`);
    const angle = Math.atan2(ey - cy, ex - cx);
    const head = Math.min(19, Math.max(10, Math.hypot(ex - sx, ey - sy) * .3));
    const tip = offset => [ex - head * Math.cos(angle + offset), ey - head * Math.sin(angle + offset)];
    root.querySelector(".tutorial-arrowhead").setAttribute("d", `M ${tip(-.55).join(" ")} L ${ex} ${ey} L ${tip(.55).join(" ")}`);
  }

  function openProfile() {
    if (isOwnPage) show("ateliers", true);
    else {
      remember("ateliers");
      window.location.assign(login.href);
    }
  }

  help.addEventListener("click", () => show(viewer ? "profile" : search ? loginStep() : "login", true));
  nextButton.addEventListener("click", () => {
    if (step === "profile") openProfile();
    else if (step === "qualifications") finish();
    else show(memberSteps[memberSteps.indexOf(step) + 1], true);
  });
  previous.addEventListener("click", () => show(memberSteps[memberSteps.indexOf(step) - 1], true));
  root.querySelector(".tutorial-close").addEventListener("click", finish);
  dismiss.addEventListener("click", finish);
  missing.addEventListener("click", () => show("membership", true));
  back.addEventListener("click", () => show("search", true));
  login?.addEventListener("click", event => {
    if (!isGuiding()) return;
    if (!viewer) remember("search");
    else if (step === "profile") {
      remember("ateliers");
      if (isOwnPage && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey && event.button === 0) {
        event.preventDefault();
        openProfile();
      }
    }
  });
  if (!viewer) {
    membership?.addEventListener("click", () => { if (isGuiding() && step === "membership") finish(); });
    document.addEventListener("powpow:login-selected", () => { if (isGuiding()) show("send", true); });
    document.addEventListener("powpow:login-sent", () => { if (isGuiding()) show("inbox", true); });
    document.addEventListener("powpow:login-search", () => { if (isGuiding() && step !== "search") show("search"); });
    document.addEventListener("powpow:login-results", () => { if (isGuiding() && step === "search") { layout(); reveal(); } });
  }
  document.addEventListener("keydown", event => {
    if (event.key === "Escape" && !root.hidden) { event.preventDefault(); finish(); }
  });
  window.addEventListener("resize", () => { if (!root.hidden) revealNavigationTarget(); scheduleLayout(); });
  window.addEventListener("scroll", scheduleLayout, {passive: true});
  window.visualViewport?.addEventListener("resize", () => { layout(); reveal(); });
  window.visualViewport?.addEventListener("scroll", scheduleLayout);
  window.addEventListener("storage", event => { if (event.key === key && event.newValue === "done") hide(); });
  window.addEventListener("pageshow", event => {
    if (savedStep() === "done") hide();
    if (event.persisted) fetch(prefix + "/api/me").then(async response => {
      if (response.status === 401 && viewer) window.location.reload();
      else if (response.ok) {
        const current = await response.json();
        if (current.id !== viewer?.id || current.membership_only) window.location.reload();
      }
    }).catch(() => {});
  });
  const observer = new ResizeObserver(scheduleLayout);
  [card, login, search, membership, send, success, ateliers, planning, qualifications, help, menu].filter(Boolean).forEach(element => observer.observe(element));

  const saved = savedStep();
  if (saved !== "done") {
    if (viewer) show(isOwnPage && memberSteps.includes(saved) ? saved : "profile");
    else if (search || document.getElementById("hero")) {
      show(search ? (saved === "membership" && loginStep() === "search" ? "membership" : loginStep()) : "login");
    }
  }
}
