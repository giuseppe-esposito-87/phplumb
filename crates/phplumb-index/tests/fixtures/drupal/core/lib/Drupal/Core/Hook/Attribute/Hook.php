<?php
namespace Drupal\Core\Hook\Attribute;

#[\Attribute]
class Hook
{
  public function __construct(public string $hook, public string $method = '', public ?string $module = NULL) {}
}
