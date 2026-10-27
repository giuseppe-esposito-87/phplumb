<?php

/**
 * Performs periodic actions.
 */
function /*@target:cron_impl*/hook_cron() {}

function /*@target:install*/hook_install() {}

function hook_example()
{
    \Drupal::service('example.only_in_docs');
}

function /*@target:update_n*/hook_update_N(&$sandbox) {}

function /*@target:post_update*/hook_post_update_NAME(&$sandbox) {}
