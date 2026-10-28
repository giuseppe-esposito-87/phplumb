<?php

namespace App\Controller;

use Symfony\Bridge\Twig\Attribute\Template;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Twig\Environment;

class PagesController extends AbstractController
{
    public function __construct(private Environment $twig)
    {
    }

    public function pages(): void
    {
        $this->render('@Blog/list.html.twig');
        $this->renderView('base.html.twig');
        $this->twig->render('@Blog/card.html.twig');
        $this->render('missing.html.twig');
    }

    #[Template('@Dev/tools.html.twig')]
    public function tools(): array
    {
        return [];
    }
}
