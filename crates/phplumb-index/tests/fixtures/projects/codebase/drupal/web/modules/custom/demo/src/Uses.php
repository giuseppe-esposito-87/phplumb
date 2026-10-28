<?php

namespace Drupal\demo;

function uses(): void
{
    \Drupal::service('demo.thing')->run();
    \Drupal::service('missing.service');
}
