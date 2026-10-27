<?php
namespace Drupal\mymodule\Entity;

use Drupal\Core\Entity\Attribute\ContentEntityType;
use Drupal\Core\Entity\ContentEntityBase;
use Drupal\mymodule\ThingStorage;

#[ContentEntityType(
  id: 'thing',
  label: new TranslatableMarkup('Thing'),
  handlers: [
    'storage' => ThingStorage::class,
  ],
)]
class Thing extends ContentEntityBase
{
    public function thingName()
    {
    }
}
