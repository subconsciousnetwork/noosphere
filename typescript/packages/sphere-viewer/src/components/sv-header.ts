import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { sharedStyles } from '../styles/shared.js';
import { store } from '../state/store.js';

@customElement('sv-header')
export class SVHeader extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.slug')
  slug?: string;

  @watch('sphereViewer.sphereViewerVersion')
  version?: string;

  @watch('sphereViewer.sphereViewerSha')
  sha?: string;

  static styles = [
    sharedStyles,
    css`
      .slug {
        font-size: 0.85em;
        font-weight: bold;
        color: var(--color-text-secondary);
      }
    `,
  ];

  render() {
    if (!this.sphereId || !this.sphereVersion) {
      return html``;
    }

    let headerContent;

    if (this.slug) {
      headerContent = html`
        <h1 class="label">
          <a href="?id=${this.sphereId}&version=${this.sphereVersion}"
            >Sphere index</a
          >
        </h1>

        <span class="slug">/${this.slug}</span>
      `;
    } else {
      headerContent = html` <h1 class="label">Sphere index</h1> `;
    }

    return html`<header class="card-header">
      <div class="card-nav nav">
        <div>
          <img class="block" src="./noosphere.svg" width="64" height="64" />
        </div>
        <div class="small color-secondary">Noosphere Lite Client</div>
        <div class="nav-end">
          <span class="capsule small color-secondary"
            ><b>v${this.version}</b>/${this.sha}</span
          >
        </div>
      </div>
      ${headerContent}
    </header>`;
  }
}
