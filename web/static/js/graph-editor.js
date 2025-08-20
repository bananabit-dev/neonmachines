import * as d3 from 'd3';

class GraphEditor {
    constructor(selector) {
        this.svg = d3.select(selector);
        this.width = this.svg.node().getBoundingClientRect().width;
        this.height = this.svg.node().getBoundingClientRect().height;

        this.nodes = [];
        this.links = [];
        this.selectedNode = null;
        this.linking = false;

        this.simulation = d3.forceSimulation(this.nodes)
            .force("charge", d3.forceManyBody().strength(-1000))
            .force("link", d3.forceLink(this.links).id(d => d.id).distance(100))
            .force("center", d3.forceCenter(this.width / 2, this.height / 2))
            .on("tick", this.ticked.bind(this));

        this.svg.append("g").attr("class", "links");
        this.svg.append("g").attr("class", "nodes");

        this.setupEventListeners();
    }

    setupEventListeners() {
        d3.select("#add-node-btn").on("click", () => this.addNode());
        d3.select("#connect-btn").on("click", () => this.startLinking());
        d3.select("#save-graph-btn").on("click", () => this.saveGraph());
        d3.select("#load-graph-btn").on("click", () => this.loadGraph());
    }

    startLinking() {
        this.linking = true;
    }

    addNode() {
        const id = `node-${this.nodes.length + 1}`;
        const newNode = { id, x: this.width / 2, y: this.height / 2, name: 'New Node' };
        this.nodes.push(newNode);
        this.update();
    }

    addLink(source, target) {
        this.links.push({ source, target });
        this.update();
    }

    update() {
        // Links
        this.link = this.svg.select(".links")
            .selectAll("line")
            .data(this.links, d => `${d.source.id}-${d.target.id}`)
            .join("line")
            .attr("stroke", "#999")
            .attr("stroke-opacity", 0.6);

        // Nodes
        this.node = this.svg.select(".nodes")
            .selectAll("g")
            .data(this.nodes, d => d.id)
            .join("g")
            .call(this.drag(this.simulation))
            .on('click', (event, d) => {
                if (this.linking) {
                    if (this.selectedNode && this.selectedNode !== d) {
                        this.addLink(this.selectedNode, d);
                        this.linking = false;
                        this.selectedNode = null;
                    } else {
                        this.selectedNode = d;
                    }
                } else {
                    this.showNodeProperties(d);
                }
            });

        this.node.selectAll('circle').remove();
        this.node.selectAll('text').remove();

        this.node.append("circle")
            .attr("r", 10)
            .attr("fill", "#69b3a2");

        this.node.append("text")
            .text(d => d.name)
            .attr("x", 12)
            .attr("y", 3);

        this.simulation.nodes(this.nodes);
        this.simulation.force("link").links(this.links);
        this.simulation.alpha(1).restart();
    }

    ticked() {
        this.link
            .attr("x1", d => d.source.x)
            .attr("y1", d => d.source.y)
            .attr("x2", d => d.target.x)
            .attr("y2", d => d.target.y);

        this.node
            .attr("transform", d => `translate(${d.x},${d.y})`);
    }

    drag(simulation) {
        function dragstarted(event, d) {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            d.fx = d.x;
            d.fy = d.y;
        }

        function dragged(event, d) {
            d.fx = event.x;
            d.fy = event.y;
        }

        function dragended(event, d) {
            if (!event.active) simulation.alphaTarget(0);
            d.fx = null;
            d.fy = null;
        }

        return d3.drag()
            .on("start", dragstarted)
            .on("drag", dragged)
            .on("end", dragended);
    }

    showNodeProperties(nodeData) {
        const propertiesContent = d3.select("#properties-content");
        propertiesContent.html(""); // Clear previous content

        for (const key in nodeData) {
            if (nodeData.hasOwnProperty(key)) {
                const value = nodeData[key];
                const propertyRow = propertiesContent.append("div");
                propertyRow.append("label").text(key);
                const input = propertyRow.append("input")
                    .attr("type", "text")
                    .attr("value", value)
                    .on("change", (event) => {
                        nodeData[key] = event.target.value;
                        this.update();
                    });
                
                if (key === 'id' || key === 'x' || key === 'y' || key === 'vx' || key === 'vy' || key === 'fx' || key === 'fy' || key === 'index') {
                    input.attr('disabled', true);
                }
            }
        }
    }

    saveGraph() {
        const graphData = {
            nodes: this.nodes.map(n => ({ id: n.id, name: n.name, x: n.x, y: n.y })),
            links: this.links.map(l => ({ source: l.source.id, target: l.target.id }))
        };
        const blob = new Blob([JSON.stringify(graphData, null, 2)], { type: "application/json;charset=utf-8" });
        const link = document.createElement("a");
        link.href = URL.createObjectURL(blob);
        link.download = "graph.json";
        link.click();
    }

    loadGraph() {
        const input = document.createElement("input");
        input.type = "file";
        input.onchange = e => {
            const file = e.target.files[0];
            const reader = new FileReader();
            reader.onload = readerEvent => {
                const content = readerEvent.target.result;
                const graphData = JSON.parse(content);
                this.nodes = graphData.nodes;
                this.links = graphData.links.map(l => ({
                    source: this.nodes.find(n => n.id === l.source),
                    target: this.nodes.find(n => n.id === l.target)
                }));
                this.update();
            }
            reader.readAsText(file);
        }
        input.click();
    }
}

document.addEventListener('DOMContentLoaded', () => {
    new GraphEditor('#graph-canvas');
});
