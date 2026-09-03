/* PowPow — extracted JavaScript */
"use strict";

// === Shared utilities ===

function showNotification(message, type) {
  const container = document.getElementById("notification-container");
  if (!container) return;
  const notification = document.createElement("div");
  notification.className = "notification is-" + type + " is-loading";
  notification.innerHTML = '<button class="delete"></button>' + message;
  container.appendChild(notification);
  notification.querySelector(".delete").addEventListener("click", function () {
    notification.remove();
  });
  setTimeout(function () {
    notification.remove();
  }, 3000);
}

function updateNameFields(form, firstName, lastName) {
  form.querySelector('input[name="first_name"]').value = firstName;
  form.querySelector('input[name="last_name"]').value = lastName;
}

function updateEmailField(form, email) {
  form.querySelector('input[name="email"]').value = email;
}

function updateFileName(input) {
  var fileName = input.files[0]
    ? input.files[0].name
    : "Aucun fichier sélectionné";
  document.getElementById("file-name").textContent = fileName;
}

// === Page initializers ===

document.addEventListener("DOMContentLoaded", function () {
  var prefix = document.body.dataset.prefix || "";

  initNavbar();
  initBadgeCounts(prefix);
  initLoginCheck(prefix);
  initHeroSlideshow(prefix);
  initStaffCarousel(prefix);
  initEquipmentBars();
  initSearchFilter();
  initPersonDetail(prefix);
  initCalendarView(prefix);
  initCalendarEditor(prefix);
  scrollCalendarToToday();
  initLoginPage(prefix);
  initQualificationsPage(prefix);
  initValidationPage(prefix);
  initPhotoPage(prefix);
});

// --- Block 1: Navbar burger toggle ---
function initNavbar() {
  var burger = document.querySelector(".navbar-burger");
  if (!burger) return;
  var menu = document.getElementById(burger.dataset.target);
  if (!menu) return;
  burger.addEventListener("click", function () {
    burger.classList.toggle("is-active");
    menu.classList.toggle("is-active");
  });
}

// --- Block: Hero slideshow ---
function initHeroSlideshow(prefix) {
  var container = document.querySelector(".hero-slides");
  if (!container) return;

  var photos;
  try {
    photos = JSON.parse(container.dataset.photos || "[]");
  } catch (e) {
    photos = [];
  }
  if (photos.length === 0) {
    // No photos — show a plain gradient background
    container.style.background =
      "linear-gradient(135deg, #2d395c 0%, #4a6fa5 100%)";
    return;
  }

  var pfx = container.dataset.prefix || "";

  // Shuffle so each page load shows a different sequence (Fisher-Yates)
  for (var i = photos.length - 1; i > 0; i--) {
    var j = Math.floor(Math.random() * (i + 1));
    var tmp = photos[i];
    photos[i] = photos[j];
    photos[j] = tmp;
  }

  // Create slide elements
  photos.forEach(function (photo, i) {
    var slide = document.createElement("div");
    slide.className = "hero-slide" + (i === 0 ? " is-active" : "");
    slide.style.backgroundImage = "url(" + pfx + "/photos/" + photo.id + ")";
    container.appendChild(slide);
  });

  // Create a single credit overlay above the hero overlay
  var credit = document.createElement("span");
  credit.className = "photo-credit hero-credit";
  credit.textContent = photos[0].name ? "\u00A9 " + photos[0].name : "";
  container.parentElement.appendChild(credit);

  if (photos.length <= 1) return;

  // Rotate slides
  var current = 0;
  var slides = container.querySelectorAll(".hero-slide");
  setInterval(function () {
    slides[current].classList.remove("is-active");
    current = (current + 1) % slides.length;
    slides[current].classList.add("is-active");
    credit.textContent = photos[current].name
      ? "\u00A9 " + photos[current].name
      : "";
  }, 5000);

  // Parallax effect: move slides at half scroll speed
  var hero = document.getElementById("hero");
  if (hero) {
    window.addEventListener(
      "scroll",
      function () {
        var scrollY = window.scrollY;
        if (scrollY < hero.offsetHeight) {
          var offset = Math.round(scrollY * 0.4);
          slides.forEach(function (s) {
            s.style.transform = "translateY(" + offset + "px)";
          });
        }
      },
      { passive: true },
    );
  }
}

// --- Block: Equipment progress bar scroll animation ---
function initEquipmentBars() {
  var bars = document.querySelectorAll(".equip-bar-fill[data-progress]");
  if (bars.length === 0) return;

  if ("IntersectionObserver" in window) {
    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            var bar = entry.target;
            bar.style.width = bar.dataset.progress + "%";
            observer.unobserve(bar);
          }
        });
      },
      { threshold: 0.2 },
    );

    bars.forEach(function (bar) {
      observer.observe(bar);
    });
  } else {
    // Fallback: animate immediately
    bars.forEach(function (bar) {
      bar.style.width = bar.dataset.progress + "%";
    });
  }
}

// --- Block: Fullscreen image modal ---
(function () {
  var modal = document.getElementById("img-modal");
  if (!modal) return;
  var modalImg = modal.querySelector(".img-modal-content");

  document.querySelectorAll(".img-modal-trigger").forEach(function (trigger) {
    trigger.addEventListener("click", function (e) {
      e.preventDefault();
      modalImg.src = trigger.dataset.src || trigger.querySelector("img").src;
      modal.classList.add("is-active");
    });
  });

  modal.addEventListener("click", function () {
    modal.classList.remove("is-active");
  });

  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape" && modal.classList.contains("is-active")) {
      modal.classList.remove("is-active");
    }
  });
})();

// --- Block: Contact email modal ---
(function () {
  var modal = document.getElementById("contact-modal");
  if (!modal) return;
  var openBtn = document.getElementById("open-contact-modal");
  var closeBtn = document.getElementById("close-contact-modal");
  var cancelBtn = document.getElementById("cancel-contact-modal");
  var submitBtn = document.getElementById("contact-submit");
  var form = document.getElementById("contact-form");
  var nameInput = document.getElementById("contact-name");
  var emailInput = document.getElementById("contact-email");
  var prefilled = false;

  function openModal() {
    modal.classList.add("is-active");
    if (!prefilled) {
      prefilled = true;
      var prefix = document.body.dataset.prefix || "";
      fetch(prefix + "/api/me")
        .then(function (r) {
          if (!r.ok) throw new Error();
          return r.json();
        })
        .then(function (d) {
          if (d.first_name || d.last_name) {
            nameInput.value = (
              (d.first_name || "") +
              " " +
              (d.last_name || "")
            ).trim();
          }
          if (d.email) emailInput.value = d.email;
        })
        .catch(function () {
          /* not logged in, leave fields empty */
        });
    }
  }

  function closeModal() {
    modal.classList.remove("is-active");
  }

  if (openBtn)
    openBtn.addEventListener("click", function (e) {
      e.preventDefault();
      openModal();
    });
  if (closeBtn) closeBtn.addEventListener("click", closeModal);
  if (cancelBtn) cancelBtn.addEventListener("click", closeModal);
  modal
    .querySelector(".modal-background")
    .addEventListener("click", closeModal);
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape" && modal.classList.contains("is-active"))
      closeModal();
  });

  if (submitBtn)
    submitBtn.addEventListener("click", function () {
      if (!form.reportValidity()) return;
      submitBtn.classList.add("is-loading");
      var prefix = document.body.dataset.prefix || "";
      fetch(prefix + "/contact", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: nameInput.value,
          email: emailInput.value,
          subject: document.getElementById("contact-subject").value,
          message: document.getElementById("contact-message").value,
        }),
      })
        .then(function (r) {
          return r.json().then(function (d) {
            return { ok: r.ok, data: d };
          });
        })
        .then(function (res) {
          submitBtn.classList.remove("is-loading");
          if (res.ok) {
            showNotification("Message envoyé !", "success");
            form.reset();
            closeModal();
          } else {
            showNotification(res.data.error || "Erreur", "danger");
          }
        })
        .catch(function () {
          submitBtn.classList.remove("is-loading");
          showNotification("Erreur réseau", "danger");
        });
    });
})();

// --- Block 2: Badge counts ---
function initBadgeCounts(prefix) {
  fetch(prefix + "/api/badge-counts")
    .then(function (r) {
      return r.json();
    })
    .then(function (d) {
      document.querySelectorAll(".nav-badge").forEach(function (b) {
        var c = d[b.dataset.badge];
        if (c > 0) {
          b.textContent = c;
          b.classList.remove("d-none");
        }
      });
    })
    .catch(function () {});
}

// --- Block 3: Login check / me API ---
function initLoginCheck(prefix) {
  fetch(prefix + "/api/me")
    .then(function (r) {
      if (r.ok) return r.json();
      throw 0;
    })
    .then(function (d) {
      var b = document.getElementById("login-btn");
      if (b) {
        b.innerHTML =
          '<i class="fa-solid fa-user"></i>&nbsp;' +
          d.first_name +
          " " +
          d.last_name;
        b.href = prefix + "/person/" + d.id;
        var lo = document.createElement("a");
        lo.className = "navbar-item";
        lo.href = prefix + "/logout";
        lo.innerHTML = '<i class="fa-solid fa-right-from-bracket"></i>';
        b.parentNode.insertBefore(lo, b.nextSibling);
      }
      if (d.is_admin || d.is_chief) {
        document.querySelectorAll(".navbar-admin").forEach(function (el) {
          el.style.display = "";
        });
      }
    })
    .catch(function () {});
}

// --- Block: Equipment cycle (3-state: closed → partial → open → closed) ---
function cycleEquipment(el) {
  var id = el.dataset.id;
  var prefix = el.dataset.prefix;
  var statusMap = {
    open: { cls: "is-success", label: "Ouvert" },
    closed: { cls: "is-danger", label: "Fermé" },
    partial: { cls: "is-warning", label: "Partiel" },
  };
  el.disabled = true;
  fetch(prefix + "/api/equipment/" + id, {
    method: "POST",
  })
    .then(function (r) {
      if (!r.ok) throw new Error("Erreur " + r.status);
      return r.json();
    })
    .then(function (d) {
      var info = statusMap[d.status] || statusMap.closed;
      el.className = "button is-small equip-cycle-btn " + info.cls;
      el.textContent = info.label;
      el.dataset.status = d.status;
    })
    .catch(function (err) {
      alert("Erreur: " + err.message);
    })
    .finally(function () {
      el.disabled = false;
    });
}

// --- Block 4: Search filter enter key ---
function initSearchFilter() {
  var searchInput = document.getElementById("searchInput");
  if (!searchInput) return;
  searchInput.addEventListener("keypress", function (e) {
    if (e.key === "Enter") {
      document.getElementById("filterForm").submit();
    }
  });
}

// --- Blocks 7-10: Person detail ---
function initPersonDetail(prefix) {
  var personDataEl = document.getElementById("person-data");
  var staffId = personDataEl ? personDataEl.dataset.staffId : null;
  if (!staffId) return;

  // Block 7: Atelier checkbox handler
  document.querySelectorAll(".atelier-checkbox").forEach(function (checkbox) {
    checkbox.addEventListener("change", async function () {
      var atelierId = this.dataset.atelierId;
      var checked = this.checked;

      try {
        var response = await fetch(
          prefix + "/api/person/" + staffId + "/role",
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ atelier_id: atelierId, add: checked }),
          },
        );

        if (!response.ok) {
          var error = await response.text();
          throw new Error(error);
        }

        showNotification(
          checked ? "Atelier ajouté" : "Atelier retiré",
          "success",
        );
        setTimeout(function () {
          location.reload();
        }, 500);
      } catch (error) {
        console.error("Error:", error);
        showNotification("Erreur: " + error.message, "danger");
        this.checked = !checked;
      }
    });
  });

  // Block 8: Admin controls (validated, chief, admin/god, comment)
  var validatedCheckboxes = document.querySelectorAll(
    ".role-validated-checkbox",
  );
  validatedCheckboxes.forEach(function (checkbox) {
    checkbox.addEventListener("change", async function () {
      var atelierId = this.dataset.atelierId;
      var checked = this.checked;

      try {
        var response = await fetch(
          prefix + "/api/person/" + staffId + "/role",
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ atelier_id: atelierId, validated: checked }),
          },
        );

        if (!response.ok) {
          var error = await response.text();
          throw new Error(error);
        }

        showNotification(
          checked ? "Rôle validé" : "Validation retirée",
          "success",
        );
      } catch (error) {
        console.error("Error:", error);
        showNotification("Erreur: " + error.message, "danger");
        this.checked = !checked;
      }
    });
  });

  document
    .querySelectorAll(".role-chief-checkbox")
    .forEach(function (checkbox) {
      checkbox.addEventListener("change", async function () {
        var atelierId = this.dataset.atelierId;
        var checked = this.checked;
        var validatedCheckbox = document.querySelector(
          '.role-validated-checkbox[data-atelier-id="' + atelierId + '"]',
        );

        try {
          var response = await fetch(
            prefix + "/api/person/" + staffId + "/role",
            {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ atelier_id: atelierId, chief: checked }),
            },
          );

          if (!response.ok) {
            var error = await response.text();
            throw new Error(error);
          }

          if (validatedCheckbox) {
            if (checked) {
              validatedCheckbox.checked = true;
              validatedCheckbox.disabled = true;
            } else {
              validatedCheckbox.disabled = false;
            }
          }

          showNotification(
            checked ? "Défini comme chef" : "Chef retiré",
            "success",
          );
        } catch (error) {
          console.error("Error:", error);
          showNotification("Erreur: " + error.message, "danger");
          this.checked = !checked;
        }
      });
    });

  var adminCb = document.getElementById("admin-cb");
  var godCb = document.getElementById("god-cb");

  if (adminCb) {
    adminCb.addEventListener("change", async function () {
      if (!this.checked && godCb) {
        godCb.checked = false;
      }
      try {
        var response = await fetch(prefix + "/api/admin/flags", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            staff_id: staffId,
            is_admin: adminCb.checked,
            is_god: godCb ? godCb.checked : false,
          }),
        });
        var data = await response.json();
        if (data.success) {
          adminCb.checked = data.is_admin;
          if (godCb) godCb.checked = data.is_god;
          showNotification("Droits mis à jour", "success");
        } else {
          showNotification("Erreur: " + (data.error || "Inconnue"), "danger");
          location.reload();
        }
      } catch (error) {
        showNotification("Erreur réseau: " + error.message, "danger");
        location.reload();
      }
    });
  }

  if (godCb) {
    godCb.addEventListener("change", async function () {
      if (this.checked && adminCb) {
        adminCb.checked = true;
      }
      try {
        var response = await fetch(prefix + "/api/admin/flags", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            staff_id: staffId,
            is_admin: adminCb ? adminCb.checked : false,
            is_god: godCb.checked,
          }),
        });
        var data = await response.json();
        if (data.success) {
          if (adminCb) adminCb.checked = data.is_admin;
          godCb.checked = data.is_god;
          showNotification("Droits mis à jour", "success");
        } else {
          showNotification("Erreur: " + (data.error || "Inconnue"), "danger");
          location.reload();
        }
      } catch (error) {
        showNotification("Erreur réseau: " + error.message, "danger");
        location.reload();
      }
    });
  }

  var optoutImportCb = document.getElementById("optout-import-cb");
  var optoutWeeklyCb = document.getElementById("optout-weekly-cb");

  var saveEmailPrefs = async function () {
    try {
      var response = await fetch(prefix + "/api/my/email-preferences", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          no_import_emails: optoutImportCb ? optoutImportCb.checked : false,
          no_weekly_emails: optoutWeeklyCb ? optoutWeeklyCb.checked : false,
        }),
      });
      var data = await response.json();
      if (data.success) {
        if (optoutImportCb) optoutImportCb.checked = data.no_import_emails;
        if (optoutWeeklyCb) optoutWeeklyCb.checked = data.no_weekly_emails;
        showNotification("Préférences de mail enregistrées", "success");
      } else {
        showNotification("Erreur: " + (data.error || "Inconnue"), "danger");
        location.reload();
      }
    } catch (error) {
      showNotification("Erreur réseau: " + error.message, "danger");
      location.reload();
    }
  };

  if (optoutImportCb) {
    optoutImportCb.addEventListener("change", saveEmailPrefs);
  }
  if (optoutWeeklyCb) {
    optoutWeeklyCb.addEventListener("change", saveEmailPrefs);
  }

  var saveCommentBtn = document.getElementById("save-comment-btn");
  if (saveCommentBtn) {
    saveCommentBtn.addEventListener("click", async function () {
      var comment = document.getElementById("comment-input").value;
      var btn = this;
      btn.classList.add("is-loading");

      try {
        var response = await fetch(
          prefix + "/api/person/" + staffId + "/comment",
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ comment: comment }),
          },
        );

        if (!response.ok) {
          var error = await response.text();
          throw new Error(error);
        }

        showNotification("Commentaire enregistré", "success");
      } catch (error) {
        console.error("Error:", error);
        showNotification("Erreur: " + error.message, "danger");
      } finally {
        btn.classList.remove("is-loading");
      }
    });
  }

  // Block 9: Contact edit
  var saveContactBtn = document.getElementById("save-contact-btn");
  if (saveContactBtn) {
    saveContactBtn.addEventListener("click", async function () {
      if (!confirm("Attention à bien vérifier avant de confirmer !")) return;
      var email = document.getElementById("edit-email").value;
      var phone = document.getElementById("edit-phone").value;
      var btn = this;
      btn.classList.add("is-loading");

      try {
        var response = await fetch(
          prefix + "/api/person/" + staffId + "/contact",
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ email: email, phone: phone || null }),
          },
        );

        if (!response.ok) {
          var error = await response.text();
          throw new Error(error);
        }

        location.reload();
      } catch (error) {
        console.error("Error:", error);
        showNotification("Erreur: " + error.message, "danger");
      } finally {
        btn.classList.remove("is-loading");
      }
    });
  }

  // Block 10: Calendar presence toggle (personal)
  document.querySelectorAll(".pcal-presence-cb").forEach(function (cb) {
    cb.addEventListener("change", async function () {
      var needId = this.dataset.need;
      var staffIdVal = this.dataset.staff;
      var half = this.dataset.half;
      var value = this.checked;

      try {
        var response = await fetch(prefix + "/api/calendar/toggle", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            needs_id: needId,
            staff_id: staffIdVal,
            half: half,
            value: value,
          }),
        });

        if (!response.ok) {
          var body = await response.json().catch(function () {
            return {};
          });
          throw new Error(body.error || "Erreur serveur");
        }

        var cell = this.closest("td");
        var anyChecked = Array.from(
          cell.querySelectorAll(".pcal-presence-cb"),
        ).some(function (c) {
          return c.checked;
        });
        cell.classList.toggle("pcal-active", anyChecked);
      } catch (error) {
        console.error("Error:", error);
        showNotification("Erreur: " + error.message, "danger");
        this.checked = !value;
      }
    });
  });
}

// --- Block 12: Calendar view presence toggle ---
function initCalendarView(prefix) {
  var presenceCbs = document.querySelectorAll(".presence-cb");
  if (presenceCbs.length === 0) return;

  presenceCbs.forEach(function (cb) {
    cb.addEventListener("change", async function () {
      var needId = this.dataset.need;
      var staffId = this.dataset.staff;
      var half = this.dataset.half;
      var value = this.checked;

      try {
        var response = await fetch(prefix + "/api/calendar/toggle", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            needs_id: needId,
            staff_id: staffId,
            half: half,
            value: value,
          }),
        });

        if (!response.ok) {
          if (response.status === 403) {
            throw new Error(
              "Vous ne pouvez modifier que votre propre disponibilité",
            );
          }
          var body = await response.json().catch(function () {
            return {};
          });
          throw new Error(body.error || "Erreur serveur");
        }

        var cell = this.closest("td");
        var anyChecked = Array.from(cell.querySelectorAll(".presence-cb")).some(
          function (c) {
            return c.checked;
          },
        );
        cell.classList.toggle("cal-active", anyChecked);

        var colIndex = cell.cellIndex;
        var table = this.closest("table");
        var th = table.querySelector(
          "thead tr th:nth-child(" + (colIndex + 1) + ")",
        );
        if (th) {
          var filledFirst = 0,
            filledSecond = 0;
          table.querySelectorAll("tbody tr").forEach(function (row) {
            var c = row.cells[colIndex];
            if (c) {
              var cbs = c.querySelectorAll(".presence-cb");
              cbs.forEach(function (cb) {
                if (cb.checked && cb.dataset.half === "first") filledFirst++;
                if (cb.checked && cb.dataset.half === "second") filledSecond++;
              });
            }
          });
          var countEl = th.querySelector(".cal-day-count");
          if (countEl) {
            var spans = countEl.querySelectorAll("span");
            if (spans.length === 2) {
              var qtyMatch = spans[0].textContent.match(/\/(\d+)/);
              var qty = qtyMatch ? parseInt(qtyMatch[1]) : 0;
              var firstLabel = spans[0].textContent
                .replace(/\d+\/\d+/, "")
                .trim();
              var secondLabel = spans[1].textContent
                .replace(/\d+\/\d+/, "")
                .trim();
              spans[0].textContent = firstLabel + " " + filledFirst + "/" + qty;
              spans[1].textContent =
                secondLabel + " " + filledSecond + "/" + qty;
              spans[0].className =
                filledFirst >= qty ? "has-text-success" : "has-text-danger";
              spans[1].className =
                filledSecond >= qty ? "has-text-success" : "has-text-danger";
              var isComplete = filledFirst >= qty && filledSecond >= qty;
              th.classList.toggle("cal-complete", isComplete);
              table.querySelectorAll("tbody tr").forEach(function (row) {
                var c = row.cells[colIndex];
                if (c) c.classList.toggle("cal-complete", isComplete);
              });
            }
          }
        }
      } catch (error) {
        console.error("Error:", error);
        showNotification("Erreur: " + error.message, "danger");
        this.checked = !value;
      }
    });
  });
}

// --- Block 13: Calendar editor ---
function initCalendarEditor(prefix) {
  var dayModal = document.getElementById("day-modal");
  if (!dayModal) return;

  var ateliersEl = document.getElementById("ateliers-data");
  var editableEl = document.getElementById("editable-data");
  if (!ateliersEl || !editableEl) return;

  var ateliers = JSON.parse(ateliersEl.textContent);
  var editableAteliers = new Set(JSON.parse(editableEl.textContent));

  function formatDateTitle(dayStr) {
    var parts = dayStr.split("-");
    var dt = new Date(parts[0], parts[1] - 1, parts[2]);
    var dayNames = [
      "Dimanche",
      "Lundi",
      "Mardi",
      "Mercredi",
      "Jeudi",
      "Vendredi",
      "Samedi",
    ];
    var monthNames = [
      "janvier",
      "février",
      "mars",
      "avril",
      "mai",
      "juin",
      "juillet",
      "août",
      "septembre",
      "octobre",
      "novembre",
      "décembre",
    ];
    return (
      dayNames[dt.getDay()] +
      " " +
      dt.getDate() +
      " " +
      monthNames[dt.getMonth()] +
      " " +
      dt.getFullYear()
    );
  }

  function renderCardsInto(container, targetDay, dayNeedsMap) {
    container.innerHTML = "";
    for (var i = 0; i < ateliers.length; i++) {
      var atelier = ateliers[i];
      var existing = dayNeedsMap[atelier.id] || null;
      var hasNeed = !!existing;
      var qty = existing ? existing.quantity : 0;
      var nightly = existing ? existing.nightly : atelier.default_nightly;

      var canEdit = editableAteliers.has(atelier.id);
      var card = document.createElement("div");
      card.className = "atelier-card" + (hasNeed ? " has-need" : "");
      card.dataset.atelierId = atelier.id;

      if (canEdit) {
        card.innerHTML =
          '<div class="card-title">' +
          atelier.name +
          "</div>" +
          '<div class="field"><label class="label is-small">Bénévoles nécessaires</label>' +
          '<div class="control"><input class="input is-small card-qty" type="number" min="0" value="' +
          qty +
          '"></div></div>' +
          '<div class="field"><div class="nightly-switch">' +
          '<span class="side-label' +
          (nightly ? "" : " is-active") +
          '" data-role="day"><i class="fa-solid fa-sun"></i> Journée</span>' +
          '<label class="switch"><input type="checkbox" class="nightly-cb"' +
          (nightly ? " checked" : "") +
          '><span class="check"></span></label>' +
          '<span class="side-label' +
          (nightly ? " is-active" : "") +
          '" data-role="night"><i class="fa-solid fa-moon"></i> Nocturne</span>' +
          "</div></div>" +
          '<div class="card-actions">' +
          '<button class="button is-primary is-small btn-card-save"><span class="icon is-small"><i class="fa-solid fa-floppy-disk"></i></span><span>' +
          (hasNeed ? "Modifier" : "Créer") +
          "</span></button>" +
          (hasNeed
            ? '<button class="button is-danger is-small is-outlined btn-card-delete"><span class="icon is-small"><i class="fa-solid fa-trash"></i></span><span>Supprimer</span></button>'
            : "") +
          "</div>";
      } else {
        card.innerHTML =
          '<div class="card-title">' +
          atelier.name +
          "</div>" +
          '<div class="field"><label class="label is-small">Bénévoles nécessaires</label>' +
          '<div class="control"><input class="input is-small card-qty" type="number" min="0" value="' +
          qty +
          '" disabled></div></div>' +
          '<div class="field"><div class="nightly-switch">' +
          '<span class="side-label' +
          (nightly ? "" : " is-active") +
          '"><i class="fa-solid fa-sun"></i> Journée</span>' +
          '<label class="switch"><input type="checkbox" class="nightly-cb"' +
          (nightly ? " checked" : "") +
          ' disabled><span class="check"></span></label>' +
          '<span class="side-label' +
          (nightly ? " is-active" : "") +
          '"><i class="fa-solid fa-moon"></i> Nocturne</span>' +
          "</div></div>";
      }

      if (canEdit) {
        (function (card, atelier, targetDay) {
          var cb = card.querySelector(".nightly-cb");
          var lblDay = card.querySelector('[data-role="day"]');
          var lblNight = card.querySelector('[data-role="night"]');
          function syncLabels() {
            lblDay.classList.toggle("is-active", !cb.checked);
            lblNight.classList.toggle("is-active", cb.checked);
          }
          cb.addEventListener("change", syncLabels);
          lblDay.addEventListener("click", function () {
            cb.checked = false;
            syncLabels();
          });
          lblNight.addEventListener("click", function () {
            cb.checked = true;
            syncLabels();
          });

          card
            .querySelector(".btn-card-save")
            .addEventListener("click", async function () {
              var q = parseInt(card.querySelector(".card-qty").value);
              var n = cb.checked;
              if (!q || q < 1) {
                showNotification("Quantité invalide", "warning");
                return;
              }
              try {
                var resp = await fetch(prefix + "/api/calendar/needs", {
                  method: "POST",
                  headers: { "Content-Type": "application/json" },
                  body: JSON.stringify({
                    atelier_id: atelier.id,
                    day: targetDay,
                    quantity: q,
                    nightly: n,
                  }),
                });
                if (!resp.ok) throw new Error(await resp.text());
                showNotification(atelier.name + " enregistré", "success");
                location.reload();
              } catch (err) {
                showNotification("Erreur: " + err.message, "danger");
              }
            });

          var delBtn = card.querySelector(".btn-card-delete");
          if (delBtn) {
            delBtn.addEventListener("click", async function () {
              if (
                !confirm(
                  "Supprimer le besoin pour " +
                    atelier.name +
                    "\u00a0? Les présences associées seront aussi supprimées.",
                )
              )
                return;
              try {
                var resp = await fetch(prefix + "/api/calendar/needs", {
                  method: "DELETE",
                  headers: { "Content-Type": "application/json" },
                  body: JSON.stringify({
                    atelier_id: atelier.id,
                    day: targetDay,
                  }),
                });
                if (!resp.ok) throw new Error(await resp.text());
                showNotification(atelier.name + " supprimé", "success");
                location.reload();
              } catch (err) {
                showNotification("Erreur: " + err.message, "danger");
              }
            });
          }
        })(card, atelier, targetDay);
      }

      container.appendChild(card);
    }
  }

  // ========== 1. Table cell click -> day-editor modal ==========
  document.querySelectorAll(".cal-table td.day-cell").forEach(function (cell) {
    cell.addEventListener("click", function () {
      var day = cell.dataset.day;
      document.getElementById("day-modal-title").textContent =
        formatDateTitle(day);
      dayModal.classList.add("is-active");
      fetch(prefix + "/api/calendar/needs-by-day?day=" + day)
        .then(function (r) {
          if (!r.ok) throw new Error();
          return r.json();
        })
        .then(function (needs) {
          var map = {};
          for (var i = 0; i < needs.length; i++)
            map[needs[i].atelier] = needs[i];
          renderCardsInto(
            document.getElementById("day-atelier-cards"),
            day,
            map,
          );
        })
        .catch(function () {
          showNotification("Erreur chargement", "danger");
        });
    });
  });
  document
    .getElementById("close-day-modal")
    .addEventListener("click", function () {
      dayModal.classList.remove("is-active");
    });
  dayModal
    .querySelector(".modal-background")
    .addEventListener("click", function () {
      dayModal.classList.remove("is-active");
    });

  // ========== 2. "Ajouter" button -> calendar-picker modal ==========
  var addModal = document.getElementById("add-modal");
  var calendarInitialised = false;
  var needDaysSet = new Set();
  var addSelectedDay = null;

  document
    .getElementById("open-add-modal")
    .addEventListener("click", function () {
      addModal.classList.add("is-active");
      if (!calendarInitialised) {
        calendarInitialised = true;
        requestAnimationFrame(function () {
          initCalendar();
        });
      } else {
        fetchNeedDays();
      }
    });
  document
    .getElementById("close-add-modal")
    .addEventListener("click", function () {
      addModal.classList.remove("is-active");
    });
  addModal
    .querySelector(".modal-background")
    .addEventListener("click", function () {
      addModal.classList.remove("is-active");
    });

  function initCalendar() {
    var calendars = bulmaCalendar.attach("#calendar-widget", {
      displayMode: "inline",
      type: "date",
      lang: "fr",
      dateFormat: "YYYY-MM-DD",
      showHeader: false,
      showFooter: false,
    });
    if (calendars.length > 0) {
      calendars[0].on("select", function (e) {
        var dt = e.data.date.start;
        if (dt) {
          var y = dt.getFullYear();
          var m = String(dt.getMonth() + 1).padStart(2, "0");
          var d = String(dt.getDate()).padStart(2, "0");
          addSelectedDay = y + "-" + m + "-" + d;
          fetchAddDayNeeds();
        }
      });
    }

    var calContainer =
      addModal.querySelector(".datetimepicker") ||
      document.querySelector("#calendar-widget").parentElement;
    if (calContainer) {
      var observer = new MutationObserver(function () {
        highlightDates();
      });
      observer.observe(calContainer, { childList: true, subtree: true });
    }

    fetchNeedDays();
  }

  function fetchAddDayNeeds() {
    document.getElementById("add-no-selection").style.display = "none";
    document.getElementById("add-edit-panel").classList.remove("d-none");
    document.getElementById("add-panel-title").textContent =
      formatDateTitle(addSelectedDay);
    fetch(prefix + "/api/calendar/needs-by-day?day=" + addSelectedDay)
      .then(function (r) {
        if (!r.ok) throw new Error();
        return r.json();
      })
      .then(function (needs) {
        var map = {};
        for (var i = 0; i < needs.length; i++) map[needs[i].atelier] = needs[i];
        renderCardsInto(
          document.getElementById("add-atelier-cards"),
          addSelectedDay,
          map,
        );
      })
      .catch(function () {
        showNotification("Erreur chargement", "danger");
      });
  }

  function fetchNeedDays() {
    fetch(prefix + "/api/calendar/need-days")
      .then(function (r) {
        if (!r.ok) throw new Error();
        return r.json();
      })
      .then(function (days) {
        needDaysSet = new Set(days);
        highlightDates();
      })
      .catch(function (err) {
        console.error("Error fetching need days:", err);
      });
  }

  function highlightDates() {
    addModal.querySelectorAll(".date-item.has-need").forEach(function (el) {
      el.classList.remove("has-need");
    });
    var monthEl = addModal.querySelector(".datepicker-nav-month");
    var yearEl = addModal.querySelector(".datepicker-nav-year");
    if (!monthEl || !yearEl) return;
    var frMonths = {
      janvier: 0,
      février: 1,
      fevrier: 1,
      mars: 2,
      avril: 3,
      mai: 4,
      juin: 5,
      juillet: 6,
      août: 7,
      aout: 7,
      septembre: 8,
      octobre: 9,
      novembre: 10,
      décembre: 11,
      decembre: 11,
    };
    var month = frMonths[monthEl.textContent.trim().toLowerCase()];
    var year = parseInt(yearEl.textContent.trim());
    if (month === undefined || isNaN(year)) return;
    addModal
      .querySelectorAll(".datepicker-body .datepicker-date")
      .forEach(function (dc) {
        if (
          dc.classList.contains("is-disabled") ||
          !dc.classList.contains("is-current-month")
        )
          return;
        var btn = dc.querySelector(".date-item");
        if (!btn) return;
        var dayNum = parseInt(btn.textContent);
        if (!dayNum || isNaN(dayNum)) return;
        var dateKey =
          year +
          "-" +
          String(month + 1).padStart(2, "0") +
          "-" +
          String(dayNum).padStart(2, "0");
        if (needDaysSet.has(dateKey)) btn.classList.add("has-need");
      });
  }

  // ========== 3. Opening day modal ==========
  var openingDayModal = document.getElementById("opening-day-modal");
  var openingDayBtn = document.getElementById("open-add-opening-day-modal");
  var openingDayConfirm = document.getElementById("opening-day-confirm");
  var openingDayConfirmText = document.getElementById(
    "opening-day-confirm-text",
  );
  var openingDaySubmit = document.getElementById("opening-day-submit");
  var closeOpeningDayModal = document.getElementById("close-opening-day-modal");
  var openingCalendarInit = false;
  var openingSelectedDay = null;

  if (openingDayBtn) {
    openingDayBtn.addEventListener("click", function () {
      openingDayModal.classList.add("is-active");
      openingSelectedDay = null;
      if (openingDayConfirm) openingDayConfirm.style.display = "none";
      if (!openingCalendarInit) {
        openingCalendarInit = true;
        requestAnimationFrame(function () {
          initOpeningCalendar();
        });
      }
    });
  }

  function initOpeningCalendar() {
    var cals = bulmaCalendar.attach("#opening-day-picker", {
      displayMode: "inline",
      type: "date",
      lang: "fr",
      dateFormat: "YYYY-MM-DD",
      showHeader: false,
      showFooter: false,
    });
    if (cals.length > 0) {
      cals[0].on("select", function (e) {
        var dt = e.data.date.start;
        if (dt) {
          var y = dt.getFullYear();
          var m = String(dt.getMonth() + 1).padStart(2, "0");
          var d = String(dt.getDate()).padStart(2, "0");
          openingSelectedDay = y + "-" + m + "-" + d;
          openingDayConfirmText.textContent =
            "Jour d'ouverture le " + formatDateTitle(openingSelectedDay) + " ?";
          openingDayConfirm.style.display = "block";
        }
      });
    }
  }

  if (openingDaySubmit) {
    openingDaySubmit.addEventListener("click", function () {
      if (!openingSelectedDay) return;
      openingDaySubmit.classList.add("is-loading");
      fetch(prefix + "/api/calendar/opening-day", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ day: openingSelectedDay }),
      })
        .then(function (resp) {
          if (!resp.ok)
            return resp.json().then(function (d) {
              throw new Error(d.error || "Erreur");
            });
          return resp.json();
        })
        .then(function (data) {
          showNotification(
            "Jour d'ouverture le " +
              formatDateTitle(openingSelectedDay) +
              " (" +
              data.needs_created +
              " besoins)",
            "success",
          );
          setTimeout(function () {
            location.reload();
          }, 800);
        })
        .catch(function (err) {
          showNotification(err.message, "danger");
          openingDaySubmit.classList.remove("is-loading");
        });
    });
  }

  if (closeOpeningDayModal) {
    closeOpeningDayModal.addEventListener("click", function () {
      openingDayModal.classList.remove("is-active");
    });
  }
  if (openingDayModal) {
    openingDayModal
      .querySelector(".modal-background")
      .addEventListener("click", function () {
        openingDayModal.classList.remove("is-active");
      });
  }

  // ========== 4. Go / NoGo modal ==========
  var gonogoModal = document.getElementById("gonogo-modal");
  var gonogoTitle = document.getElementById("gonogo-title");
  var closeGonogoModal = document.getElementById("close-gonogo-modal");
  var gonogoCancel = document.getElementById("gonogo-cancel");
  var gonogoGo = document.getElementById("gonogo-go");
  var gonogoNogo = document.getElementById("gonogo-nogo");
  var gonogoDay = "";

  document.querySelectorAll(".opening-tag").forEach(function (tag) {
    tag.addEventListener("click", function () {
      gonogoDay = tag.dataset.day;
      gonogoTitle.textContent = formatDateTitle(gonogoDay);
      gonogoModal.classList.add("is-active");
    });
  });

  function closeGonogo() {
    gonogoModal.classList.remove("is-active");
    gonogoDay = "";
  }

  if (closeGonogoModal) closeGonogoModal.addEventListener("click", closeGonogo);
  if (gonogoCancel) gonogoCancel.addEventListener("click", closeGonogo);
  if (gonogoModal) {
    gonogoModal
      .querySelector(".modal-background")
      .addEventListener("click", closeGonogo);
  }

  function sendGonogoStatus(status) {
    if (!gonogoDay) return;
    var btn = status === "validated" ? gonogoGo : gonogoNogo;
    btn.classList.add("is-loading");
    fetch(prefix + "/api/calendar/opening-day/status", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ day: gonogoDay, status: status }),
    })
      .then(function (resp) {
        if (!resp.ok)
          return resp.json().then(function (d) {
            throw new Error(d.error || "Erreur");
          });
        return resp.json();
      })
      .then(function () {
        var label = status === "validated" ? "Confirmé" : "Annulé";
        showNotification(
          label + " : " + formatDateTitle(gonogoDay),
          status === "validated" ? "success" : "warning",
        );
        setTimeout(function () {
          location.reload();
        }, 800);
      })
      .catch(function (err) {
        showNotification(err.message, "danger");
        btn.classList.remove("is-loading");
      });
  }

  if (gonogoGo)
    gonogoGo.addEventListener("click", function () {
      sendGonogoStatus("validated");
    });
  if (gonogoNogo)
    gonogoNogo.addEventListener("click", function () {
      sendGonogoStatus("canceled");
    });
}

// --- Block 14: Login page ---
function initLoginPage(prefix) {
  var input = document.getElementById("search-input");
  if (!input) return;

  var panel = document.getElementById("results-panel");
  var confirmBox = document.getElementById("confirm-box");
  var confirmText = document.getElementById("confirm-text");
  var sendBtn = document.getElementById("send-btn");
  var successBox = document.getElementById("success-box");
  var errorBox = document.getElementById("error-box");
  var errorText = document.getElementById("error-text");
  var debounceTimer = null;
  var selectedStaff = null;

  input.addEventListener("input", function () {
    clearTimeout(debounceTimer);
    confirmBox.style.display = "none";
    successBox.style.display = "none";
    errorBox.style.display = "none";
    selectedStaff = null;
    var q = input.value.trim();
    if (q.length < 4) {
      panel.style.display = "none";
      panel.innerHTML = "";
      return;
    }
    debounceTimer = setTimeout(function () {
      fetch(prefix + "/api/staff/search?q=" + encodeURIComponent(q))
        .then(function (r) {
          return r.json();
        })
        .then(function (data) {
          panel.innerHTML = "";
          if (data.length === 0) {
            panel.innerHTML = '<p class="panel-block">Aucun résultat</p>';
          } else {
            data.forEach(function (s) {
              var a = document.createElement("a");
              a.className = "panel-block";
              a.textContent = s.first_name + " " + s.last_name;
              a.href = "#";
              a.addEventListener("click", function (e) {
                e.preventDefault();
                selectedStaff = s;
                confirmText.textContent =
                  "Envoyer un email de connexion à " +
                  s.first_name +
                  " " +
                  s.last_name +
                  " ?";
                confirmBox.style.display = "block";
                successBox.style.display = "none";
                errorBox.style.display = "none";
              });
              panel.appendChild(a);
            });
          }
          panel.style.display = "block";
        })
        .catch(function () {
          panel.innerHTML = '<p class="panel-block">Erreur de recherche</p>';
          panel.style.display = "block";
        });
    }, 300);
  });

  sendBtn.addEventListener("click", function () {
    if (!selectedStaff) return;
    sendBtn.classList.add("is-loading");
    errorBox.style.display = "none";
    fetch(prefix + "/api/login/send", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ staff_id: selectedStaff.id }),
    })
      .then(function (r) {
        return r.json();
      })
      .then(function (data) {
        sendBtn.classList.remove("is-loading");
        if (data.success) {
          confirmBox.style.display = "none";
          panel.style.display = "none";
          successBox.style.display = "block";
          input.value = "";
        } else {
          errorText.textContent = data.error || "Erreur inconnue";
          errorBox.style.display = "block";
        }
      })
      .catch(function () {
        sendBtn.classList.remove("is-loading");
        errorText.textContent = "Erreur réseau";
        errorBox.style.display = "block";
      });
  });
}

// --- Block 14b: Qualifications page staff search ---
function initQualificationsPage(prefix) {
  var input = document.getElementById("sq-staff-search");
  if (!input) return;

  var panel = document.getElementById("sq-staff-results");
  var hiddenInput = document.getElementById("sq-staff-id");
  var selectedBox = document.getElementById("sq-staff-selected");
  var selectedName = document.getElementById("sq-staff-selected-name");
  var debounceTimer = null;

  input.addEventListener("input", function () {
    clearTimeout(debounceTimer);
    hiddenInput.value = "";
    selectedBox.style.display = "none";
    var q = input.value.trim();
    if (q.length < 4) {
      panel.style.display = "none";
      panel.innerHTML = "";
      return;
    }
    debounceTimer = setTimeout(function () {
      fetch(prefix + "/api/staff/search?q=" + encodeURIComponent(q))
        .then(function (r) {
          return r.json();
        })
        .then(function (data) {
          panel.innerHTML = "";
          if (data.length === 0) {
            panel.innerHTML = '<p class="panel-block">Aucun résultat</p>';
          } else {
            data.forEach(function (s) {
              var a = document.createElement("a");
              a.className = "panel-block";
              a.textContent = s.first_name + " " + s.last_name;
              a.href = "#";
              a.addEventListener("click", function (e) {
                e.preventDefault();
                hiddenInput.value = s.id;
                selectedName.textContent = s.first_name + " " + s.last_name;
                selectedBox.style.display = "block";
                panel.style.display = "none";
                input.value = s.first_name + " " + s.last_name;
              });
              panel.appendChild(a);
            });
          }
          panel.style.display = "block";
        })
        .catch(function () {
          panel.innerHTML = '<p class="panel-block">Erreur de recherche</p>';
          panel.style.display = "block";
        });
    }, 300);
  });
}

// --- Block 15: Validation page ---
function initValidationPage(prefix) {
  // doValidate is called from onclick attributes in the HTML
  if (!document.querySelector('[onclick*="doValidate"]')) return;
  // Make doValidate global since it's called from inline onclick
  // (already defined below as a global function)
}

async function doValidate(staffId, atelierId, accept) {
  var prefix = document.body.dataset.prefix || "";
  var row = document.getElementById("row-" + staffId + "-" + atelierId);
  if (!row) return;
  try {
    if (accept) {
      var r = await fetch(prefix + "/api/person/" + staffId + "/role", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ atelier_id: atelierId, validated: true }),
      });
      if (!r.ok) throw new Error("Erreur validation");
    } else {
      var r2 = await fetch(prefix + "/api/person/" + staffId + "/role", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ atelier_id: atelierId, add: false }),
      });
      if (!r2.ok) throw new Error("Erreur suppression");
    }
    row.remove();
    var tbody = document.querySelector("tbody");
    if (tbody && tbody.children.length === 0) {
      tbody.innerHTML =
        '<tr><td colspan="3" class="has-text-centered has-text-grey-light py-5">Aucune demande en attente de validation</td></tr>';
    }
  } catch (e) {
    alert(e.message || "Erreur");
  }
}

// --- Block 16: Photo page ---
function initPhotoPage(prefix) {
  var searchInput = document.getElementById("photographer-search");
  if (!searchInput) return;

  // File input name display
  document.querySelectorAll('input[type="file"]').forEach(function (input) {
    input.addEventListener("change", function (e) {
      var fileName = e.target.files[0]
        ? e.target.files[0].name
        : "Aucun fichier sélectionné";
      var fileNameSpan = e.target.closest(".file").querySelector(".file-name");
      if (fileNameSpan) fileNameSpan.textContent = fileName;
    });
  });

  var resultsPanel = document.getElementById("photographer-results");
  var hiddenInput = document.getElementById("photographer_id");
  var selectedBox = document.getElementById("photographer-selected");
  var selectedTag = document.getElementById("photographer-selected-tag");
  var clearBtn = document.getElementById("photographer-clear");
  var createBox = document.getElementById("create-staff-box");
  var createBtn = document.getElementById("create-staff-btn");
  var createError = document.getElementById("create-staff-error");
  var uploadBtn = document.getElementById("upload-btn");
  var debounceTimer = null;

  function selectStaff(id, name) {
    hiddenInput.value = id;
    selectedTag.textContent = name;
    selectedBox.style.display = "block";
    searchInput.style.display = "none";
    resultsPanel.style.display = "none";
    createBox.style.display = "none";
    uploadBtn.disabled = false;
  }

  if (clearBtn)
    clearBtn.addEventListener("click", function () {
      hiddenInput.value = "";
      selectedBox.style.display = "none";
      searchInput.style.display = "";
      searchInput.value = "";
      searchInput.focus();
      uploadBtn.disabled = true;
    });

  searchInput.addEventListener("input", function () {
    clearTimeout(debounceTimer);
    resultsPanel.style.display = "none";
    resultsPanel.innerHTML = "";
    createBox.style.display = "none";
    var q = searchInput.value.trim();
    if (q.length < 4) return;

    debounceTimer = setTimeout(function () {
      fetch(prefix + "/api/staff/search?q=" + encodeURIComponent(q))
        .then(function (r) {
          return r.json();
        })
        .then(function (data) {
          resultsPanel.innerHTML = "";
          if (data.length === 0) {
            resultsPanel.innerHTML =
              '<p class="panel-block">Aucun résultat</p>';
          } else {
            data.forEach(function (s) {
              var a = document.createElement("a");
              a.className = "panel-block";
              a.textContent = s.first_name + " " + s.last_name;
              a.href = "#";
              a.addEventListener("click", function (e) {
                e.preventDefault();
                selectStaff(s.id, s.first_name + " " + s.last_name);
              });
              resultsPanel.appendChild(a);
            });
          }
          var createLink = document.createElement("a");
          createLink.className = "panel-block has-text-info";
          createLink.href = "#";
          createLink.innerHTML =
            '<span class="icon"><i class="fa-solid fa-plus"></i></span> Créer un nouveau bénévole';
          createLink.addEventListener("click", function (e) {
            e.preventDefault();
            createBox.style.display = "block";
            createError.style.display = "none";
          });
          resultsPanel.appendChild(createLink);
          resultsPanel.style.display = "block";
        })
        .catch(function () {
          resultsPanel.innerHTML =
            '<p class="panel-block">Erreur de recherche</p>';
          resultsPanel.style.display = "block";
        });
    }, 300);
  });

  if (createBtn)
    createBtn.addEventListener("click", function () {
      var first = document.getElementById("new-staff-first").value.trim();
      var last = document.getElementById("new-staff-last").value.trim();
      var email = document.getElementById("new-staff-email").value.trim();
      var phone = document.getElementById("new-staff-phone").value.trim();
      if (!first || !last) {
        createError.textContent = "Prénom et nom requis";
        createError.style.display = "block";
        return;
      }
      createBtn.disabled = true;
      createError.style.display = "none";
      fetch(prefix + "/api/staff/create-minimal", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          first_name: first,
          last_name: last,
          email: email || undefined,
          phone: phone || undefined,
        }),
      })
        .then(function (r) {
          if (r.status === 409)
            return r.json().then(function (d) {
              throw new Error(d.error);
            });
          if (!r.ok)
            return r.json().then(function (d) {
              throw new Error(d.error || "Erreur serveur");
            });
          return r.json();
        })
        .then(function (s) {
          selectStaff(s.id, s.first_name + " " + s.last_name);
          createBtn.disabled = false;
        })
        .catch(function (err) {
          createError.textContent = err.message;
          createError.style.display = "block";
          createBtn.disabled = false;
        });
    });

  var form = document.getElementById("photo-upload-form");
  if (form)
    form.addEventListener("submit", function (e) {
      e.preventDefault();
      if (!hiddenInput.value) {
        alert("Veuillez sélectionner un photographe");
        return;
      }
      var fileInput = form.querySelector('input[type="file"]');
      if (!fileInput.files.length) {
        alert("Veuillez sélectionner une photo");
        return;
      }
      uploadBtn.disabled = true;
      uploadBtn.querySelector("span:last-child").textContent =
        "Envoi en cours...";
      var formData = new FormData(form);
      fetch(form.action, {
        method: "POST",
        body: formData,
        credentials: "same-origin",
      })
        .then(function (r) {
          if (r.redirected) {
            window.location.href = r.url;
            return;
          }
          if (!r.ok) {
            return r.text().then(function (t) {
              throw new Error(
                "Erreur " + r.status + ": " + t.substring(0, 200),
              );
            });
          }
          window.location.href = prefix + "/photos";
        })
        .catch(function (err) {
          alert("Échec upload: " + err.message);
          uploadBtn.disabled = false;
          uploadBtn.querySelector("span:last-child").textContent =
            "Télécharger";
        });
    });

  // Frontpage toggle checkboxes
  document.querySelectorAll(".frontpage-toggle").forEach(function (cb) {
    cb.addEventListener("change", function () {
      var photoId = cb.dataset.id;
      fetch(prefix + "/api/photos/" + photoId + "/frontpage", {
        method: "POST",
      })
        .then(function (r) {
          return r.json();
        })
        .then(function (data) {
          if (data.is_frontpage !== undefined) {
            cb.checked = data.is_frontpage;
          }
        })
        .catch(function () {
          cb.checked = !cb.checked; // revert on error
        });
    });
  });

  // Staff toggle checkboxes
  document.querySelectorAll(".staff-toggle").forEach(function (cb) {
    cb.addEventListener("change", function () {
      var photoId = cb.dataset.id;
      fetch(prefix + "/api/photos/" + photoId + "/staff", { method: "POST" })
        .then(function (r) {
          return r.json();
        })
        .then(function (data) {
          if (data.is_staff !== undefined) {
            cb.checked = data.is_staff;
          }
        })
        .catch(function () {
          cb.checked = !cb.checked; // revert on error
        });
    });
  });
}

// --- Block: Staff photo carousel ---
function initStaffCarousel(prefix) {
  var container = document.querySelector(".staff-carousel");
  if (!container) return;

  var photos;
  try {
    photos = JSON.parse(container.dataset.photos || "[]");
  } catch (e) {
    photos = [];
  }
  if (photos.length === 0) {
    container.style.display = "none";
    return;
  }

  var pfx = container.dataset.prefix || prefix || "";
  var track = container.querySelector(".staff-carousel-track");
  var prevBtn = container.querySelector(".staff-prev");
  var nextBtn = container.querySelector(".staff-next");

  // Create slide wrappers with img + credit overlay
  photos.forEach(function (photo) {
    var wrapper = document.createElement("div");
    wrapper.className = "staff-carousel-slide";
    var img = document.createElement("img");
    img.src = pfx + "/photos/" + photo.id;
    img.alt = "Staff";
    img.loading = "lazy";
    wrapper.appendChild(img);
    if (photo.name) {
      var credit = document.createElement("span");
      credit.className = "photo-credit";
      credit.textContent = "\u00A9 " + photo.name;
      wrapper.appendChild(credit);
    }
    track.appendChild(wrapper);
  });

  if (photos.length <= 1) {
    prevBtn.style.display = "none";
    nextBtn.style.display = "none";
    return;
  }

  var current = 0;
  var total = photos.length;

  function goTo(idx) {
    current = (idx + total) % total;
    track.style.transform = "translateX(-" + current * 100 + "%)";
  }

  prevBtn.addEventListener("click", function () {
    goTo(current - 1);
  });
  nextBtn.addEventListener("click", function () {
    goTo(current + 1);
  });

  // Auto-advance every 4 seconds
  var timer = setInterval(function () {
    goTo(current + 1);
  }, 4000);

  // Pause auto-advance on hover
  container.addEventListener("mouseenter", function () {
    clearInterval(timer);
  });
  container.addEventListener("mouseleave", function () {
    timer = setInterval(function () {
      goTo(current + 1);
    }, 4000);
  });
}

// --- Scroll calendar to today's column ---
function scrollCalendarToToday() {
  var scroll = document.querySelector(".cal-scroll");
  if (!scroll) return;
  // Find today's column, or fall back to the first non-past column
  var target =
    scroll.querySelector("thead .cal-today") ||
    scroll.querySelector("thead th:not(.cal-past):not(.cal-name-col)");
  if (!target) return;
  // Wait for layout, then scroll so target is visible near the left
  setTimeout(function () {
    // Use offsetLeft relative to the table, which matches scrollLeft space
    var table = scroll.querySelector("table");
    if (!table) return;
    var targetLeft = target.offsetLeft;
    var nameCol = scroll.querySelector(".cal-name-col");
    var nameWidth = nameCol ? nameCol.offsetWidth : 0;
    scroll.scrollLeft = Math.max(0, targetLeft - nameWidth - 16);
  }, 50);
}


