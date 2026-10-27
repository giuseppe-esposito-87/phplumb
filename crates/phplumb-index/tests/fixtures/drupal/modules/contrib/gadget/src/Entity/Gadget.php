<?php
namespace Drupal\gadget\Entity;

use Drupal\Core\Entity\ContentEntityBase;

/**
 * A gadget.
 *
 * @ContentEntityType(
 *   id = "gadget",
 *   label = @Translation("Gadget"),
 *   base_table = "gadget",
 * )
 */
class Gadget extends ContentEntityBase
{
    public function gadgetName()
    {
    }
}
