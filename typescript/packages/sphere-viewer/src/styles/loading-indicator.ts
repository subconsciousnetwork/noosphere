import { css } from 'lit';

export const loadingIndicator = css`
  .loading-indicator {
    display: flex;
    width: 100%;
    height: 100%;
    flex-direction: row;
    justify-content: center;
    align-items: center;
    gap: 0.5em;
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
    animation: oscillate 1s infinite 0s, color-wheel 10s infinite, fade-in 1s;
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
`;
