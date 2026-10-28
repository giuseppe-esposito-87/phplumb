<?php

namespace App;

function uses(): void
{
    \Drupal::service('demo.thing')->run();
    \Drupal::service('missing.service');
}
