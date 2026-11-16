    /* ===========================================
       SLIDE PRESENTATION CONTROLLER
       Handles fixed-stage scaling and all navigation methods.
       =========================================== */
    class SlidePresentation {
      constructor() {
        this.slides = [...document.querySelectorAll('.slide')];
        this.currentSlide = 0;
        this.stage = document.getElementById('deckStage');
        this.slideCount = document.getElementById('slideCount');
        this.progressBar = document.getElementById('progressBar');
        this.touchStartX = null;
        this.wheelLocked = false;

        this.setupStageScale();
        this.setupKeyboardNavigation();
        this.setupTouchNavigation();
        this.setupWheelNavigation();
        this.setupButtons();
        this.showSlide(0);
      }

      setupStageScale() {
        const scaleStage = () => {
          const factor = Math.min(window.innerWidth / 1920, window.innerHeight / 1080);
          const x = (window.innerWidth - 1920 * factor) / 2;
          const y = (window.innerHeight - 1080 * factor) / 2;
          this.stage.style.transform = `translate(${x}px, ${y}px) scale(${factor})`;
        };
        scaleStage();
        window.addEventListener('resize', scaleStage);
      }

      setupKeyboardNavigation() {
        document.addEventListener('keydown', (event) => {
          if (event.target instanceof HTMLElement && event.target.isContentEditable) return;
          if (['ArrowRight', 'ArrowDown', ' ', 'PageDown'].includes(event.key)) {
            event.preventDefault();
            this.next();
          }
          if (['ArrowLeft', 'ArrowUp', 'PageUp'].includes(event.key)) {
            event.preventDefault();
            this.previous();
          }
          if (event.key === 'Home') this.showSlide(0);
          if (event.key === 'End') this.showSlide(this.slides.length - 1);
        });
      }

      setupTouchNavigation() {
        document.addEventListener('touchstart', (event) => {
          this.touchStartX = event.changedTouches[0].clientX;
        }, { passive: true });
        document.addEventListener('touchend', (event) => {
          if (this.touchStartX === null) return;
          const delta = event.changedTouches[0].clientX - this.touchStartX;
          if (Math.abs(delta) > 45) delta < 0 ? this.next() : this.previous();
          this.touchStartX = null;
        }, { passive: true });
      }

      setupWheelNavigation() {
        document.addEventListener('wheel', (event) => {
          if (this.wheelLocked || Math.abs(event.deltaY) < 18) return;
          this.wheelLocked = true;
          event.deltaY > 0 ? this.next() : this.previous();
          window.setTimeout(() => { this.wheelLocked = false; }, 500);
        }, { passive: true });
      }

      setupButtons() {
        document.getElementById('previousSlide').addEventListener('click', () => this.previous());
        document.getElementById('nextSlide').addEventListener('click', () => this.next());
      }

      next() {
        this.showSlide(this.currentSlide + 1);
      }

      previous() {
        this.showSlide(this.currentSlide - 1);
      }

      showSlide(index) {
        this.currentSlide = Math.max(0, Math.min(index, this.slides.length - 1));
        this.slides.forEach((slide, slideIndex) => {
          const isCurrent = slideIndex === this.currentSlide;
          slide.classList.toggle('active', isCurrent);
          slide.classList.toggle('visible', isCurrent);
          slide.setAttribute('aria-hidden', String(!isCurrent));
          const video = slide.querySelector('video');
          if (video instanceof HTMLVideoElement) {
            if (isCurrent) {
              video.currentTime = 0;
              void video.play().catch(() => {});
            } else {
              video.pause();
            }
          }
        });
        this.slideCount.textContent = `${this.currentSlide + 1} / ${this.slides.length}`;
        this.progressBar.style.width = `${((this.currentSlide + 1) / this.slides.length) * 100}%`;
      }
    }

    /* ===========================================
       INLINE TEXT EDITOR
       Press E or use the top-left hotzone. Changes persist locally.
       =========================================== */
    class InlineEditor {
      constructor() {
        this.isActive = false;
        this.elements = [...document.querySelectorAll('[data-editable]')];
        this.storageKey = 'openhome-expo-presentation-edits';
        this.restore();
      }

      toggleEditMode() {
        this.isActive = !this.isActive;
        document.body.classList.toggle('editing', this.isActive);
        document.getElementById('editToggle').classList.toggle('active', this.isActive);
        this.elements.forEach((element) => {
          element.contentEditable = String(this.isActive);
        });
        if (!this.isActive) this.save();
      }

      save() {
        const values = this.elements.map((element) => element.innerHTML);
        localStorage.setItem(this.storageKey, JSON.stringify(values));
      }

      restore() {
        const stored = localStorage.getItem(this.storageKey);
        if (stored === null) return;
        try {
          const values = JSON.parse(stored);
          if (!Array.isArray(values)) return;
          this.elements.forEach((element, index) => {
            if (typeof values[index] === 'string') element.innerHTML = values[index];
          });
        } catch {
          localStorage.removeItem(this.storageKey);
        }
      }
    }

    const presentation = new SlidePresentation();
    const editor = new InlineEditor();

    /* ===========================================
       EDITOR INTERACTIONS
       JavaScript grace period prevents the edit button disappearing on hover.
       =========================================== */
    const hotzone = document.querySelector('.edit-hotzone');
    const editToggle = document.getElementById('editToggle');
    let hideTimeout = null;

    hotzone.addEventListener('mouseenter', () => {
      window.clearTimeout(hideTimeout);
      editToggle.classList.add('show');
    });
    hotzone.addEventListener('mouseleave', () => {
      hideTimeout = window.setTimeout(() => {
        if (!editor.isActive) editToggle.classList.remove('show');
      }, 400);
    });
    editToggle.addEventListener('mouseenter', () => window.clearTimeout(hideTimeout));
    editToggle.addEventListener('mouseleave', () => {
      hideTimeout = window.setTimeout(() => {
        if (!editor.isActive) editToggle.classList.remove('show');
      }, 400);
    });
    hotzone.addEventListener('click', () => editor.toggleEditMode());
    editToggle.addEventListener('click', () => editor.toggleEditMode());

    document.addEventListener('keydown', (event) => {
      const target = event.target;
      const editingText = target instanceof HTMLElement && target.isContentEditable;
      if ((event.key === 'e' || event.key === 'E') && !editingText) editor.toggleEditMode();
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        editor.save();
      }
    });
