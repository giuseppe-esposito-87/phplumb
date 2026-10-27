<?php
namespace Drupal\mymodule\Plugin\Block;

use Drupal\Core\Block\Attribute\Block;

#[Block(id: 'menu_block', deriver: MenuDeriver::class)]
class MenuBlock
{
}
