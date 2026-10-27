<?php
namespace Drupal\mymodule\Plugin\Block;

use Drupal\Core\Block\Attribute\Block;

#[Block(id: 'entity:thing', deriver: EntityThingDeriver::class)]
class EntityThingBlock
{
}
