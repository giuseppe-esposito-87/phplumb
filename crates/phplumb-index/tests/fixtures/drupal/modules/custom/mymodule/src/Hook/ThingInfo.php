<?php
namespace Drupal\mymodule\Hook;

use Drupal\Core\Hook\Attribute\Hook;

#[Hook('thing_info')]
class ThingInfo
{
    public function __invoke(): void
    {
    }
}
