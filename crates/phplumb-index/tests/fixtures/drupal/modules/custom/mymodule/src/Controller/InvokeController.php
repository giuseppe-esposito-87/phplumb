<?php
namespace Drupal\mymodule\Controller;

use Symfony\Component\Routing\Attribute\Route;

#[Route('/invoke', name: 'mymodule.invoke')]
class InvokeController
{
    public function __invoke()
    {
    }
}
