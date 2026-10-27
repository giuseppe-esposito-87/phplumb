<?php
namespace Drupal\override\Entity;

/**
 * @ContentEntityType(
 *   id = "dual",
 *   handlers = {
 *     "storage" = "Drupal\override\OverrideStorage",
 *   },
 * )
 */
class DualOverride extends \Drupal\Core\Entity\ContentEntityBase
{
}
