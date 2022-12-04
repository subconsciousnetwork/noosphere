import './sphere-content.js';
import './sphere-footer.js';
import './sphere-index.js';

import { customElement } from 'lit/decorators.js';
import { LitElement, html, css } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { store } from './state/store.js';

import { sharedStyles } from './shared-styles.js';

@customElement('sphere-viewer')
export class SphereViewer extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.sphereViewerVersion')
  version?: string;

  @watch('sphereViewer.sphereViewerSha')
  sha?: string;

  @watch('sphereViewer.slug')
  slug?: string;

  @watch('sphereViewer.fileContents')
  fileContents?: string;

  @watch('sphereViewer.loading')
  loading?: boolean;

  static styles = [
    sharedStyles,
    css`
      .body-content {
        display: block;
        min-height: 8em;
      }

      .body-content.message {
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
      }

      .loading-indicator {
        display: flex;
        flex-direction: row;
        justify-content: space-between;
        width: 3em;
      }

      .loading-indicator:before,
      .loading-indicator:after,
      .loading-indicator > span {
        content: '';
        display: block;
        width: 0.75em;
        height: 0.75em;
        border-radius: 0.75em;
        color: transparent;
        -webkit-user-select: none;
        -moz-user-select: none;
        -ms-user-select: none;
        user-select: none;

        animation: oscillate 1s infinite;
      }

      .loading-indicator:before {
        animation: oscillate 1s infinite 0s, color-wheel 10s infinite,
          fade-in 1s;
      }

      .loading-indicator > span {
        animation: oscillate 1s infinite -0.33s, color-wheel 10s infinite,
          fade-in 1s;
      }

      .loading-indicator:after {
        animation: oscillate 1s infinite -0.66s, color-wheel 10s infinite,
          fade-in 1s;
      }

      @keyframes fade-in {
        0% {
          opacity: 0;
        }
        100% {
          opacity: 1;
        }
      }

      @keyframes color-wheel {
        0% {
          background: #67fff5;
        }

        33% {
          background: #8557b3;
        }

        66% {
          background: #f197c1;
        }

        100% {
          background: #67fff5;
        }
      }

      @keyframes oscillate {
        0% {
          transform: translateY(-30%);
        }

        50% {
          transform: translateY(30%);
        }

        100% {
          transform: translateY(-30%);
        }
      }
    `,
  ];

  render() {
    let bodyContent;

    if (this.loading) {
      bodyContent = html`<div class="card-body center body-content message">
        <div class="loading-indicator"><span>Loading...</span></div>
      </div>`;
    } else if (this.sphereId && this.sphereVersion) {
      if (this.slug) {
        bodyContent = html`<sphere-content></sphere-content>`;
      } else {
        bodyContent = html`<sphere-index></sphere-index>`;
      }
    } else {
      bodyContent = html`<div class="card-body center body-content message">
        <p>No sphere address information specified</p>
      </div>`;
    }

    return html`
      <div class="container pad-container">
        <article class="card">
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
          ${bodyContent}
          <sphere-footer></sphere-footer>
        </article>
      </div>
    `;
    // return html` ${body} ${footer} `;
  }
}
