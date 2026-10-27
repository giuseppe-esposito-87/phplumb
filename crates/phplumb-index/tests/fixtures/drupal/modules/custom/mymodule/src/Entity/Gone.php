<?php
namespace Drupal\mymodule\Entity;

use Drupal\Core\Entity\Attribute\ContentEntityType;
use Drupal\mymodule\MissingStorage;

#[ContentEntityType(id: 'gone', handlers: ['storage' => MissingStorage::class])]
class Gone extends \Drupal\Core\Entity\ContentEntityBase
{
}
