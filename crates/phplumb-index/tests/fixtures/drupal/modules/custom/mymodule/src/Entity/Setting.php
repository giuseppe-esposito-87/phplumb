<?php
namespace Drupal\mymodule\Entity;

use Drupal\Core\Config\Entity\ConfigEntityBase;
use Drupal\Core\Entity\Attribute\ConfigEntityType;

#[ConfigEntityType(id: 'setting')]
class Setting extends ConfigEntityBase
{
}

#[ContentEntityType(id: 'dual')]
class Dual extends \Drupal\Core\Entity\ContentEntityBase
{
}
