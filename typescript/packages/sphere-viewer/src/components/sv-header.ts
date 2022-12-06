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
      .icon-back {
        display: block;
        width: 32px;
        height: 32px;
      }

      .icon-back:after {
        content: '';
        display: block;
        width: 50%;
        height: 50%;
        border: 3px solid purple;
        border-bottom-width: 0;
        border-right-width: 0;
        transform-origin: top right;
        transform: translate(10%, 10%) rotate(-45deg);
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
        <header class="card-header">
          <a
            href="?id=${this.sphereId}&version=${this.sphereVersion}&slug=${this
              .slug}"
            ><span class="slug">/${this.slug}</span></a
          >
        </header>
      `;
    } else {
      headerContent = html``;
    }

    let backButton;

    if (this.slug) {
      backButton = html`
        <a
          class="icon-back"
          href="?id=${this.sphereId}&version=${this.sphereVersion}"
        ></a>
      `;
    } else {
      backButton = html``;
    }

    return html`<nav class="card-nav nav">
        <div class="nav-start">${backButton}</div>
        <div class="flex justify-center">
          <img class="block" src="./noosphere.svg" width="64" height="64" />
        </div>
        <div class="flex justify-end">
          <span class="capsule small color-secondary"
            ><b>v${this.version}</b>/${this.sha}</span
          >
        </div>
      </nav>
      ${headerContent} `;
  }
}
